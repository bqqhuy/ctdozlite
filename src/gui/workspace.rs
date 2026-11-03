use super::*;

impl ZaloGuiApp {
    fn sort_conversation_indices(&self, indices: &mut [usize]) {
        // Compute each activity timestamp once: history may be evicted or delivered out of order.
        // Cloud has no implicit pin; explicitly pinned threads retain the user's chosen priority.
        indices.sort_by_cached_key(|&index| {
            let thread = &self.threads[index];
            (
                std::cmp::Reverse(
                    self.conversation_preferences
                        .get(&thread.id)
                        .is_some_and(|p| p.pinned),
                ),
                std::cmp::Reverse(thread.sidebar_activity_ts()),
                thread.id.as_str(),
            )
        });
    }

    pub(super) fn reset_sync_status(&mut self) {
        self.quote_jump = None;
        self.phone_sync_running = false;
        self.phone_sync_status.clear();
        self.phone_download_status.clear();
        self.phone_import_status.clear();
        self.history_sync_running = false;
        self.history_sync_status.clear();
        self.refresh_sync_running = false;
        self.refresh_sync_status.clear();
        self.sync_cancel_requested = false;
        self.sync_notice_dismissed = false;
        self.last_sync_was_history = true;
    }

    fn sync_status_message(&self) -> &str {
        if self.phone_sync_running
            || (!self.phone_sync_status.is_empty()
                && !self.history_sync_running
                && !self.refresh_sync_running)
        {
            &self.phone_sync_status
        } else if self.history_sync_running {
            &self.history_sync_status
        } else if self.refresh_sync_running || !self.last_sync_was_history {
            &self.refresh_sync_status
        } else {
            &self.history_sync_status
        }
    }

    pub(super) fn render_sync_status_bar(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let running =
            self.phone_sync_running || self.history_sync_running || self.refresh_sync_running;
        if !self.is_logged_in
            || (!running && (self.sync_notice_dismissed || self.sync_status_message().is_empty()))
        {
            return;
        }
        egui::TopBottomPanel::bottom("main_sync_status")
            .resizable(false)
            .exact_height(92.0)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_app)
                    .inner_margin(Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if running {
                        ui.spinner();
                    }
                    ui.strong(if self.phone_sync_running || self.sync_status_message()==self.phone_sync_status && !self.phone_sync_status.is_empty() {"Đồng bộ từ điện thoại"}else{"Đồng bộ từ máy chủ"});
                    if (self.phone_sync_running || self.history_sync_running)
                        && ui
                            .add_enabled(
                                !self.sync_cancel_requested,
                                egui::Button::new(if self.sync_cancel_requested {
                                    "Đang dừng…"
                                } else {
                                    "Dừng"
                                }),
                            )
                            .clicked()
                    {
                        if let Some(tx) = WORKER_TX.get() {
                            if tx.send(if self.phone_sync_running {WorkerCmd::CancelPhoneHistory}else{WorkerCmd::CancelHistorySync}).is_ok() {
                                self.sync_cancel_requested = true;
                            }
                        }
                    }
                    if self.phone_sync_running
                        && !self.sync_cancel_requested
                        && ui
                            .button("Đánh thức điện thoại")
                            .on_hover_text("Gửi lại thông báo tới Zalo trên điện thoại khi việc gửi lịch sử bị đứng")
                            .clicked()
                    {
                        crate::phone_sync::request_wake();
                    }
                    if ui.button("Chi tiết").clicked() {
                        self.settings_tab = SettingsTab::Sync;
                        self.settings_modal_open = true;
                    }
                    if !running && ui.button("Ẩn trạng thái").clicked() {
                        self.sync_notice_dismissed = true;
                    }
                });
                let phone=self.phone_sync_running || (!self.phone_sync_status.is_empty() && !self.history_sync_running && !self.refresh_sync_running);
                let download=if phone && !self.phone_download_status.is_empty(){self.phone_download_status.as_str()}else{self.sync_status_message()};
                ui.add(egui::Label::new(RichText::new(download).color(pal.text_primary)).truncate()).on_hover_text(download);
                let importing=if phone && !self.phone_import_status.is_empty(){self.phone_import_status.as_str()}else{"Tiến độ chi tiết nằm trong mục Đồng bộ"};
                ui.add(egui::Label::new(RichText::new(importing).color(pal.text_secondary)).truncate()).on_hover_text(importing);

            });
    }

    pub(super) fn render_chat_workspace(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        // Responsive breakpoints based on live viewport width
        // -------------------------------------------------------------------
        let screen_rect = ctx.screen_rect();
        let screen_w = screen_rect.width();
        let is_compact_w = screen_w < 780.0;
        let is_narrow_w = screen_w < 620.0;
        let left_rail_w = if is_narrow_w { 52.0 } else { 62.0 };

        // -------------------------------------------------------------------
        // 1. LEFT NAVIGATION RAIL (Replaces top bar & bottom bar)
        // -------------------------------------------------------------------
        egui::SidePanel::left("left_nav_rail")
            .resizable(false)
            .exact_width(left_rail_w)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_app)
                    .inner_margin(Margin::symmetric(if is_narrow_w { 4.0 } else { 8.0 }, 14.0))
                    .stroke(Stroke::new(1.0_f32, pal.border)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // User Avatar Button with Cutout Online Status Dot
                    let (av_rect, av_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    let user_disp = if self.user_name.is_empty() {
                        "Người dùng Zalo"
                    } else {
                        &self.user_name
                    };
                    paint_user_avatar(
                        ui.painter(),
                        av_rect.center(),
                        20.0,
                        user_disp,
                        false,
                        self.user_avatar.as_deref(),
                        &self.sticker_textures,
                        &mut self.loading_stickers,
                    );
                    if av_resp.hovered() || self.profile_menu_open {
                        ui.painter().circle_stroke(
                            av_rect.center(),
                            21.0,
                            Stroke::new(2.0_f32, pal.primary_hover),
                        );
                    }
                    // Cutout status dot
                    if self.settings.show_online_status {
                        let dot_c = Pos2::new(av_rect.right() - 4.0, av_rect.bottom() - 4.0);
                        ui.painter().circle_filled(dot_c, 6.5, pal.bg_app);
                        ui.painter().circle_filled(dot_c, 4.5, pal.accent_online);
                    }

                    if av_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(format!(
                            "{} (Đang trực tuyến — Bấm để mở menu tài khoản)",
                            if self.user_name.is_empty() {
                                "Tài khoản Zalo"
                            } else {
                                &self.user_name
                            }
                        ))
                        .clicked()
                    {
                        self.profile_menu_open = !self.profile_menu_open;
                    }

                    ui.add_space(18.0);

                    // Navigation Icon 1: Tin nhắn (All / Active Chat)
                    let is_msg_nav = self.thread_filter == ThreadFilter::All && !self.contacts_view_open;
                    let (nav1_rect, nav1_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_msg_nav {
                        ui.painter().rect_filled(
                            nav1_rect,
                            Rounding::same(11.0),
                            pal.primary.linear_multiply(0.18),
                        );
                        let pill = Rect::from_min_size(
                            Pos2::new(nav1_rect.left() - 6.0, nav1_rect.center().y - 10.0),
                            Vec2::new(3.5, 20.0),
                        );
                        ui.painter()
                            .rect_filled(pill, Rounding::same(2.0), pal.primary);
                    } else if nav1_resp.hovered() {
                        ui.painter()
                            .rect_filled(nav1_rect, Rounding::same(11.0), pal.bg_hover);
                    }
                    draw_icon_chat_nav(
                        ui.painter(),
                        nav1_rect.center(),
                        19.0,
                        if is_msg_nav {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );
                    if nav1_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Tin nhắn")
                        .clicked()
                    {
                        self.contacts_view_open = false;
                        self.thread_filter = ThreadFilter::All;
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 2: Danh bạ (friends and groups in one page)
                    let is_contacts_nav = self.contacts_view_open;
                    let (nav2_rect, nav2_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_contacts_nav {
                        ui.painter().rect_filled(
                            nav2_rect,
                            Rounding::same(11.0),
                            pal.primary.linear_multiply(0.18),
                        );
                        let pill = Rect::from_min_size(
                            Pos2::new(nav2_rect.left() - 6.0, nav2_rect.center().y - 10.0),
                            Vec2::new(3.5, 20.0),
                        );
                        ui.painter()
                            .rect_filled(pill, Rounding::same(2.0), pal.primary);
                    } else if nav2_resp.hovered() {
                        ui.painter()
                            .rect_filled(nav2_rect, Rounding::same(11.0), pal.bg_hover);
                    }
                    draw_icon_contacts(
                        ui.painter(),
                        nav2_rect.center(),
                        19.0,
                        if is_contacts_nav {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );
                    if nav2_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Danh bạ")
                        .clicked()
                    {
                        self.contacts_view_open = !self.contacts_view_open;
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 3: Cloud của tôi
                    let is_cloud_selected = !self.contacts_view_open
                        && self
                            .threads
                            .get(self.selected_thread_index)
                            .map(|t| t.name.contains("Cloud của tôi"))
                            .unwrap_or(false);
                    let (nav3_rect, nav3_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_cloud_selected {
                        ui.painter().rect_filled(
                            nav3_rect,
                            Rounding::same(11.0),
                            pal.primary.linear_multiply(0.18),
                        );
                    } else if nav3_resp.hovered() {
                        ui.painter()
                            .rect_filled(nav3_rect, Rounding::same(11.0), pal.bg_hover);
                    }
                    draw_icon_cloud(
                        ui.painter(),
                        nav3_rect.center(),
                        19.0,
                        if is_cloud_selected {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );
                    if nav3_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Cloud của tôi")
                        .clicked()
                    {
                        if let Some((cidx, cthread)) = self
                            .threads
                            .iter()
                            .enumerate()
                            .find(|(_, t)| t.name.contains("Cloud của tôi"))
                        {
                            let cid = cthread.id.clone();
                            self.contacts_view_open = false;
                            self.selected_thread_index = cidx;
                            if let Some(tx) = WORKER_TX.get() {
                                let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                    limit: 500,
                                    group_id: cid,
                                    is_group: false,
                                });
                            }
                        }
                    }

                    // Push bottom actions to the bottom of the Left Rail
                    let rem_h = (ui.available_height() - 100.0).max(12.0);
                    ui.add_space(rem_h);

                    // Bottom Action 1: Background Sync Indicator Icon
                    let (sync_rect, sync_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    if sync_resp.hovered() {
                        ui.painter()
                            .rect_filled(sync_rect, Rounding::same(10.0), pal.bg_hover);
                    }
                    draw_icon_refresh(
                        ui.painter(),
                        sync_rect.center(),
                        17.0,
                        if sync_resp.hovered() {
                            pal.accent_online
                        } else {
                            pal.text_secondary
                        },
                    );
                    ui.painter().circle_filled(
                        Pos2::new(sync_rect.right() - 9.0, sync_rect.top() + 9.0),
                        3.0,
                        pal.accent_online,
                    );
                    if sync_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(format!(
                            "Trạng thái: {} · {} hội thoại (Bấm để đồng bộ ngay)",
                            self.status_msg,
                            self.threads.len()
                        ))
                        .clicked()
                    {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::RefreshThreads);
                        }
                        self.show_toast(format!(
                            "Đang đồng bộ ngầm {} hội thoại…",
                            self.threads.len()
                        ));
                    }

                    ui.add_space(6.0);

                    // Bottom Action 2: Theme / Settings Icon
                    // Bottom Action 2: Settings Modal Button
                    let (set_rect, set_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    if self.settings_modal_open {
                        ui.painter().rect_filled(
                            set_rect,
                            Rounding::same(10.0),
                            pal.primary.linear_multiply(0.18),
                        );
                    } else if set_resp.hovered() {
                        ui.painter()
                            .rect_filled(set_rect, Rounding::same(10.0), pal.bg_hover);
                    }
                    draw_icon_settings(
                        ui.painter(),
                        set_rect.center(),
                        17.5,
                        if self.settings_modal_open {
                            pal.primary_hover
                        } else if set_resp.hovered() {
                            pal.text_primary
                        } else {
                            pal.text_secondary
                        },
                    );
                    if set_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Cài đặt Zalo")
                        .clicked()
                    {
                        self.settings_modal_open = !self.settings_modal_open;
                    }
                });
            });

        if self.contacts_view_open {
            self.render_contacts_page(ctx, pal);
            self.render_workspace_overlays(ctx, pal);
            return;
        }

        // -------------------------------------------------------------------
        // 2. CONVERSATION LIST SIDEBAR (Responsive width, auto-adapts on resize)
        // -------------------------------------------------------------------
        let sidebar_default_w = if is_narrow_w {
            76.0
        } else if is_compact_w {
            240.0
        } else {
            300.0
        };
        let (sidebar_id, sidebar_min_w, sidebar_max_w) = if is_narrow_w {
            ("conversation_sidebar_narrow", 68.0, 68.0)
        } else {
            (
                "conversation_sidebar_wide",
                190.0,
                (screen_w * 0.45).clamp(240.0, 420.0),
            )
        };

        egui::SidePanel::left(sidebar_id)
            .resizable(!is_narrow_w)
            .default_width(sidebar_default_w)
            .width_range(sidebar_min_w..=sidebar_max_w)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, pal.border)),
            )
            .show(ctx, |ui| {
                egui::Frame::none()
                    .inner_margin(Margin {
                        left: if is_narrow_w { 4.0 } else { 14.0 },
                        right: if is_narrow_w { 4.0 } else { 14.0 },
                        top: 12.0,
                        bottom: 0.0,
                    })
                    .show(ui, |ui| {
                        if is_narrow_w {
                            // Compact mode: icon-only search / lookup button
                            ui.vertical_centered(|ui| {
                                let (srch_r, srch_resp) =
                                    ui.allocate_exact_size(Vec2::new(42.0, 32.0), Sense::click());
                                ui.painter()
                                    .rect_filled(srch_r, Rounding::same(8.0), pal.bg_input);
                                ui.painter().rect_stroke(
                                    srch_r,
                                    Rounding::same(8.0),
                                    Stroke::new(1.0_f32, pal.border),
                                );
                                draw_icon_search(
                                    ui.painter(),
                                    srch_r.center(),
                                    16.0,
                                    pal.primary_hover,
                                );
                                if srch_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text("Tìm kiếm hội thoại & Tra cứu SĐT")
                                    .clicked()
                                {
                                    self.phone_modal_open = true;
                                }
                            });
                            ui.add_space(6.0);
                        } else {
                            // Search Box + Quick Phone Lookup (`+👤`) Button
                            let avail_row_w = ui.available_width();
                            let mut trigger_phone_lookup: Option<String> = None;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let search_box_w = (avail_row_w - 38.0).max(110.0);
                                let has_q = !self.search_query.is_empty();
                                egui::Frame::none()
                                    .fill(pal.bg_input)
                                    .rounding(Rounding::same(8.0))
                                    .stroke(Stroke::new(
                                        1.0_f32,
                                        if has_q {
                                            pal.primary.linear_multiply(0.65)
                                        } else {
                                            pal.border
                                        },
                                    ))
                                    .inner_margin(Margin::symmetric(9.0, 6.0))
                                    .show(ui, |ui| {
                                        let inner_w = (search_box_w - 18.0).max(80.0);
                                        ui.set_width(inner_w);
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            let (icon_rect, _) = ui.allocate_exact_size(
                                                Vec2::new(15.0, 18.0),
                                                Sense::hover(),
                                            );
                                            draw_icon_search(
                                                ui.painter(),
                                                icon_rect.center(),
                                                13.5,
                                                if has_q {
                                                    pal.primary_hover
                                                } else {
                                                    pal.text_muted
                                                },
                                            );
                                            let edit_w = if has_q {
                                                (inner_w - 42.0).max(48.0)
                                            } else {
                                                (inner_w - 22.0).max(60.0)
                                            };
                                            let s_resp = ui.add(
                                                egui::TextEdit::singleline(&mut self.search_query)
                                                    .id(Id::new("sidebar_search_input"))
                                                    .frame(false)
                                                    .hint_text("Tìm hội thoại, tin nhắn, SĐT...")
                                                    .desired_width(edit_w),
                                            );
                                            let s_enter = (s_resp.lost_focus()
                                                || s_resp.has_focus())
                                                && ui.input(|i| i.key_pressed(Key::Enter));
                                            let digits_cnt = self
                                                .search_query
                                                .chars()
                                                .filter(|c| c.is_ascii_digit())
                                                .count();
                                            if s_enter && digits_cnt >= 8 {
                                                trigger_phone_lookup =
                                                    Some(self.search_query.trim().to_string());
                                            }
                                            if has_q {
                                                let (clr_rect, clr_resp) = ui.allocate_exact_size(
                                                    Vec2::new(16.0, 16.0),
                                                    Sense::click(),
                                                );
                                                if clr_resp.hovered() {
                                                    ui.painter().circle_filled(
                                                        clr_rect.center(),
                                                        8.0,
                                                        pal.bg_hover,
                                                    );
                                                }
                                                draw_lucide_icon(
                                                    ui.painter(),
                                                    clr_rect.center(),
                                                    lucide::X,
                                                    11.5,
                                                    if clr_resp.hovered() {
                                                        pal.text_primary
                                                    } else {
                                                        pal.text_muted
                                                    },
                                                );
                                                if clr_resp
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("Xóa tìm kiếm")
                                                    .clicked()
                                                {
                                                    self.search_query.clear();
                                                }
                                            }
                                        });
                                    });

                                // Add Friend / Lookup by Phone Button (`+👤`)
                                let (add_rect, add_resp) =
                                    ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::click());
                                ui.painter().rect_filled(
                                    add_rect,
                                    Rounding::same(8.0),
                                    if self.phone_modal_open {
                                        pal.primary.linear_multiply(0.22)
                                    } else if add_resp.hovered() {
                                        pal.bg_hover
                                    } else {
                                        pal.bg_input
                                    },
                                );
                                ui.painter().rect_stroke(
                                    add_rect,
                                    Rounding::same(8.0),
                                    Stroke::new(
                                        1.0_f32,
                                        if self.phone_modal_open || add_resp.hovered() {
                                            pal.primary
                                        } else {
                                            pal.border
                                        },
                                    ),
                                );
                                draw_icon_user_plus(
                                    ui.painter(),
                                    add_rect.center(),
                                    16.5,
                                    if self.phone_modal_open || add_resp.hovered() {
                                        pal.primary_hover
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if add_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text("Tìm kiếm số điện thoại Zalo & Kết bạn")
                                    .clicked()
                                {
                                    self.phone_modal_open = true;
                                    let digits_cnt = self
                                        .search_query
                                        .chars()
                                        .filter(|c| c.is_ascii_digit())
                                        .count();
                                    if digits_cnt >= 8 && self.phone_modal_input.is_empty() {
                                        self.phone_modal_input =
                                            self.search_query.trim().to_string();
                                    }
                                }
                            });

                            if let Some(phone_str) = trigger_phone_lookup {
                                self.phone_modal_open = true;
                                self.phone_modal_input = phone_str.clone();
                                self.phone_lookup_loading = true;
                                self.phone_lookup_error = None;
                                self.phone_lookup_result = None;
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::LookupPhone { phone: phone_str });
                                }
                            }
                            ui.add_space(10.0);

                            // Clean Underline Segmented Tabs (`Tất cả`, `Cá nhân`, `Nhóm`) + `Đã ghim (1)`
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 16.0;
                                for (filter, label) in [
                                    (ThreadFilter::All, "Tất cả"),
                                    (ThreadFilter::Direct, "Cá nhân"),
                                    (ThreadFilter::Groups, "Nhóm"),
                                ] {
                                    let active = self.thread_filter == filter;
                                    let text_w = ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            label.to_string(),
                                            FontId::proportional(12.5),
                                            pal.text_primary,
                                        )
                                        .size()
                                        .x
                                    });
                                    let (tab_rect, tab_resp) = ui.allocate_exact_size(
                                        Vec2::new(text_w + 4.0, 26.0),
                                        Sense::click(),
                                    );
                                    let fg = if active {
                                        pal.primary_hover
                                    } else if tab_resp.hovered() {
                                        pal.text_primary
                                    } else {
                                        pal.text_secondary
                                    };
                                    ui.painter().text(
                                        Pos2::new(tab_rect.center().x, tab_rect.top() + 8.0),
                                        egui::Align2::CENTER_CENTER,
                                        label,
                                        FontId::proportional(12.5),
                                        fg,
                                    );
                                    if active {
                                        let line_y = tab_rect.bottom() - 1.0;
                                        ui.painter().line_segment(
                                            [
                                                Pos2::new(tab_rect.left(), line_y),
                                                Pos2::new(tab_rect.right(), line_y),
                                            ],
                                            Stroke::new(2.2_f32, pal.primary),
                                        );
                                    }
                                    if tab_resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
                                    {
                                        self.thread_filter = filter;
                                    }
                                }

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let pinned_cnt = self
                                        .threads
                                        .iter()
                                        .filter(|t| t.name.contains("Cloud của tôi"))
                                        .count();
                                    if pinned_cnt > 0 {
                                        ui.label(
                                            RichText::new(format!("Đã ghim ({pinned_cnt})"))
                                                .size(11.0)
                                                .color(pal.text_muted),
                                        );
                                    }
                                });
                            });
                        }
                    });

                ui.add(egui::Separator::default().spacing(1.0));

                // Sort threads: "Cloud của tôi" pinned first, then descending by latest message timestamp
                let query = self.search_query.trim().to_lowercase();
                let query_folded = fold_vn_search(&query);
                let query_digits: String = self
                    .search_query
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect();
                let is_phone_like_query = query_digits.len() >= 8
                    && self
                        .search_query
                        .trim()
                        .chars()
                        .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '.'));

                if is_phone_like_query {
                    let raw_q = self.search_query.trim().to_string();
                    egui::Frame::none()
                        .fill(pal.primary.linear_multiply(0.14))
                        .stroke(Stroke::new(1.0_f32, pal.primary))
                        .rounding(Rounding::same(9.0))
                        .inner_margin(Margin::symmetric(10.0, 7.0))
                        .outer_margin(Margin::symmetric(8.0, 4.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (ir, _) =
                                    ui.allocate_exact_size(Vec2::splat(20.0), Sense::hover());
                                draw_icon_user_plus(
                                    ui.painter(),
                                    ir.center(),
                                    16.0,
                                    pal.primary_hover,
                                );
                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(format!("Tìm Zalo qua SĐT: {raw_q}"))
                                            .size(12.2)
                                            .color(pal.text_primary)
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new("Nhấn nút Tìm hoặc phím Enter để tra cứu")
                                            .size(10.8)
                                            .color(pal.text_secondary),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let btn = egui::Button::new(
                                        RichText::new("Tìm SĐT")
                                            .size(11.5)
                                            .color(Color32::WHITE)
                                            .strong(),
                                    )
                                    .fill(pal.primary)
                                    .rounding(Rounding::same(6.0));
                                    if ui
                                        .add(btn)
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        self.phone_modal_open = true;
                                        self.phone_modal_input = raw_q.clone();
                                        self.phone_lookup_loading = true;
                                        self.phone_lookup_error = None;
                                        self.phone_lookup_result = None;
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ =
                                                tx.send(WorkerCmd::LookupPhone { phone: raw_q });
                                        }
                                    }
                                });
                            });
                        });
                }

                if self.conversation_preferences.values().any(|p| p.hidden) {
                    ui.checkbox(
                        &mut self.show_hidden_conversations,
                        "Hiện cả hội thoại đã ẩn",
                    );
                }
                let mut filtered_indices: Vec<usize> = self
                    .threads
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| {
                        let pass_filter = match self.thread_filter {
                            ThreadFilter::All => true,
                            ThreadFilter::Direct => !t.is_group,
                            ThreadFilter::Groups => t.is_group,
                        };
                        pass_filter
                            && (self.show_hidden_conversations
                                || !self
                                    .conversation_preferences
                                    .get(&t.id)
                                    .is_some_and(|p| p.hidden))
                            && (query.is_empty()
                                || matches_vn_query(&t.name, &query, &query_folded)
                                || t.id.contains(&query))
                    })
                    .map(|(i, _)| i)
                    .collect();

                self.sort_conversation_indices(&mut filtered_indices);

                // Also search message contents across all conversations when `query` is non-empty!
                if query.len() >= 2
                    && ui
                        .small_button("Tìm trong toàn bộ lịch sử đã lưu")
                        .clicked()
                {
                    self.tools.open = true;
                    self.tools.tab = 0;
                    self.tools.query = self.search_query.clone();
                    self.tools.last_query = self.search_query.clone();
                    self.tools_command(super::tools_panel::ToolsCommand::Search {
                        query: self.search_query.clone(),
                        hidden: self.show_hidden_conversations,
                    });
                }
                let msg_hits: Vec<(usize, String, String, String, bool)> = if query.len() >= 2 {
                    let mut hits = Vec::new();
                    for (t_idx, t) in self.threads.iter().enumerate() {
                        if !self.show_hidden_conversations
                            && self
                                .conversation_preferences
                                .get(&t.id)
                                .is_some_and(|p| p.hidden)
                        {
                            continue;
                        }
                        for m in t.messages.iter().rev() {
                            if matches_vn_query(&m.text, &query, &query_folded) {
                                hits.push((
                                    t_idx,
                                    t.name.clone(),
                                    m.text.clone(),
                                    m.display_time(),
                                    t.is_group,
                                ));
                                if hits.len() >= 25 {
                                    break;
                                }
                            }
                        }
                        if hits.len() >= 25 {
                            break;
                        }
                    }
                    hits
                } else {
                    Vec::new()
                };

                let mut preference_action = None;
                let mut clear_history_action = None;
                let mut read_action = None;
                let mut clicked_thread: Option<(usize, String, bool)> = None;
                let mut clicked_msg_hit: Option<(usize, String)> = None;

                egui::ScrollArea::vertical()
                    .id_salt("threads_list_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(4.0);
                        for &i in &filtered_indices {
                            let t = &self.threads[i];
                            let is_selected = i == self.selected_thread_index;
                            let is_user_pinned = self
                                .conversation_preferences
                                .get(&t.id)
                                .is_some_and(|p| p.pinned);

                            let row_height = if is_narrow_w { 50.0 } else { 58.0 };
                            let row_width = (ui.available_width() - 8.0).max(44.0);
                            let (row_rect, row_resp) = ui.allocate_exact_size(
                                Vec2::new(row_width, row_height),
                                Sense::click(),
                            );
                            let row_rect = row_rect.translate(Vec2::new(4.0, 0.0));
                            let preference = self
                                .conversation_preferences
                                .get(&t.id)
                                .copied()
                                .unwrap_or_default();
                            row_resp.context_menu(|ui| {
                                ui.label("Hội thoại trên máy này");
                                if ui
                                    .button(if preference.pinned {
                                        "Bỏ ghim hội thoại"
                                    } else {
                                        "Ghim hội thoại"
                                    })
                                    .clicked()
                                {
                                    preference_action = Some((
                                        t.id.clone(),
                                        t.is_group,
                                        crate::store::ConversationPreference {
                                            pinned: !preference.pinned,
                                            ..preference
                                        },
                                    ));
                                    ui.close_menu();
                                }
                                if ui
                                    .button(if preference.muted {
                                        "Bật thông báo (mọi thiết bị)"
                                    } else {
                                        "Tắt thông báo (mọi thiết bị)"
                                    })
                                    .clicked()
                                {
                                    preference_action = Some((
                                        t.id.clone(),
                                        t.is_group,
                                        crate::store::ConversationPreference {
                                            muted: !preference.muted,
                                            ..preference
                                        },
                                    ));
                                    ui.close_menu();
                                }
                                if ui
                                    .button(if preference.hidden {
                                        "Hiện hội thoại"
                                    } else {
                                        "Ẩn khỏi danh sách"
                                    })
                                    .clicked()
                                {
                                    preference_action = Some((
                                        t.id.clone(),
                                        t.is_group,
                                        crate::store::ConversationPreference {
                                            hidden: !preference.hidden,
                                            ..preference
                                        },
                                    ));
                                    ui.close_menu();
                                }
                                if ui.button("Đánh dấu đã đọc trên máy này").clicked() {
                                    read_action = Some(t.id.clone());
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("Xóa cuộc trò chuyện…").clicked(){clear_history_action=Some((t.id.clone(),true));ui.close_menu();}
                                if ui.button("Xóa lịch sử trên máy này…").clicked() {
                                    clear_history_action = Some((t.id.clone(),false));
                                    ui.close_menu();
                                }
                            });

                            if !ui.is_rect_visible(row_rect) {
                                continue;
                            }

                            let bg = if is_selected {
                                pal.bg_selected
                            } else if row_resp.hovered() {
                                pal.bg_hover
                            } else {
                                Color32::TRANSPARENT
                            };

                            ui.painter().rect_filled(row_rect, Rounding::same(11.0), bg);

                            // Left active indicator bar (`w-1 bg-zalo-500 rounded-r-full`)
                            if is_selected {
                                let bar_rect = Rect::from_min_size(
                                    Pos2::new(row_rect.left() + 1.0, row_rect.top() + 10.0),
                                    Vec2::new(3.5, 38.0),
                                );
                                ui.painter().rect_filled(
                                    bar_rect,
                                    Rounding::same(2.0),
                                    pal.primary,
                                );
                            }

                            // Avatar circle with real user/group image & Cutout Status Dot
                            let av_radius = if is_narrow_w { 18.0 } else { 20.5 };
                            let avatar_center = if is_narrow_w {
                                row_rect.center()
                            } else {
                                Pos2::new(row_rect.left() + 29.0, row_rect.center().y)
                            };
                            paint_user_avatar(
                                ui.painter(),
                                avatar_center,
                                av_radius,
                                &t.name,
                                t.is_group,
                                t.avatar.as_deref(),
                                &self.sticker_textures,
                                &mut self.loading_stickers,
                            );
                            // Presence is unknown: no verified presence feed is available.
                            // Only show text labels when sidebar is expanded
                            if !is_narrow_w {
                                // Thread Name
                                let text_left = row_rect.left() + 58.0;
                                let name_color = if is_selected && pal.is_dark {
                                    Color32::WHITE
                                } else {
                                    pal.text_primary
                                };
                                let max_name_chars = if is_compact_w { 15 } else { 21 };
                                let display_name = truncate_chars(&t.name, max_name_chars);
                                ui.painter().text(
                                    Pos2::new(text_left, row_rect.top() + 12.0),
                                    egui::Align2::LEFT_TOP,
                                    display_name,
                                    FontId::proportional(13.6),
                                    name_color,
                                );

                                // Pin icon + Timestamp (top-right)
                                let row_time = t.sidebar_timestamp(crate::login::unix_now() * 1000);
                                let time_right = row_rect.right() - 9.0;
                                if !row_time.is_empty() {
                                    let tw = ui.fonts(|f| {
                                        f.layout_no_wrap(
                                            row_time.clone(),
                                            FontId::proportional(11.0),
                                            pal.text_muted,
                                        )
                                        .size()
                                        .x
                                    });
                                    ui.painter().text(
                                        Pos2::new(time_right, row_rect.top() + 13.5),
                                        egui::Align2::RIGHT_TOP,
                                        &row_time,
                                        FontId::proportional(11.0),
                                        pal.text_muted,
                                    );
                                    if is_user_pinned {
                                        draw_icon_pin(
                                            ui.painter(),
                                            Pos2::new(time_right - tw - 9.0, row_rect.top() + 19.5),
                                            11.0,
                                            pal.text_muted,
                                        );
                                    }
                                }

                                let raw_preview = crate::message_view::preview(t);
                                let unread_n = self.unread_counts.get(&t.id).copied().unwrap_or(0);
                                let max_snip_chars = if is_compact_w {
                                    16
                                } else if unread_n > 0 {
                                    23
                                } else {
                                    28
                                };
                                let snippet = truncate_chars(&raw_preview, max_snip_chars);
                                ui.painter().text(
                                    Pos2::new(text_left, row_rect.top() + 33.5),
                                    egui::Align2::LEFT_TOP,
                                    snippet,
                                    FontId::proportional(12.0),
                                    if unread_n > 0 {
                                        pal.text_primary
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if unread_n > 0 {
                                    let badge_txt = if unread_n > 99 {
                                        "99+".to_string()
                                    } else {
                                        unread_n.to_string()
                                    };
                                    let bw = if unread_n >= 10 { 22.0 } else { 18.0 };
                                    let b_rect = Rect::from_min_size(
                                        Pos2::new(
                                            row_rect.right() - bw - 8.0,
                                            row_rect.top() + 31.5,
                                        ),
                                        Vec2::new(bw, 17.0),
                                    );
                                    ui.painter().rect_filled(
                                        b_rect,
                                        Rounding::same(8.5),
                                        pal.danger,
                                    );
                                    ui.painter().text(
                                        b_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        badge_txt,
                                        FontId::proportional(10.2),
                                        Color32::WHITE,
                                    );
                                }
                            }

                            let row_resp = row_resp.on_hover_cursor(CursorIcon::PointingHand);
                            let row_resp = if is_narrow_w {
                                row_resp.on_hover_text(&t.name)
                            } else {
                                row_resp
                            };
                            if row_resp.clicked() {
                                clicked_thread = Some((i, t.id.clone(), t.is_group));
                            }
                        }

                        // Global Message Content Matches (`Tin nhắn tìm thấy`)
                        if !msg_hits.is_empty() {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new(format!(
                                        "TIN NHẮN TÌM THẤY ({})",
                                        msg_hits.len()
                                    ))
                                    .size(11.0)
                                    .color(pal.primary_hover)
                                    .strong(),
                                );
                            });
                            ui.add_space(4.0);
                            for (t_idx, t_name, m_text, m_time, t_group) in &msg_hits {
                                let row_width = ui.available_width() - 12.0;
                                let (r_rect, r_resp) = ui.allocate_exact_size(
                                    Vec2::new(row_width, 52.0),
                                    Sense::click(),
                                );
                                let r_rect = r_rect.translate(Vec2::new(6.0, 0.0));
                                if r_resp.hovered() {
                                    ui.painter().rect_filled(
                                        r_rect,
                                        Rounding::same(10.0),
                                        pal.bg_hover,
                                    );
                                }
                                let av_c = Pos2::new(r_rect.left() + 24.0, r_rect.center().y);
                                let hit_ava =
                                    self.threads.get(*t_idx).and_then(|th| th.avatar.as_deref());
                                paint_user_avatar(
                                    ui.painter(),
                                    av_c,
                                    16.0,
                                    t_name,
                                    *t_group,
                                    hit_ava,
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );
                                let tl = r_rect.left() + 50.0;
                                ui.painter().text(
                                    Pos2::new(tl, r_rect.top() + 9.0),
                                    egui::Align2::LEFT_TOP,
                                    truncate_chars(t_name, 20),
                                    FontId::proportional(12.8),
                                    pal.text_primary,
                                );
                                if !m_time.is_empty() {
                                    ui.painter().text(
                                        Pos2::new(r_rect.right() - 10.0, r_rect.top() + 10.0),
                                        egui::Align2::RIGHT_TOP,
                                        m_time,
                                        FontId::proportional(10.5),
                                        pal.text_muted,
                                    );
                                }
                                ui.painter().text(
                                    Pos2::new(tl, r_rect.top() + 28.0),
                                    egui::Align2::LEFT_TOP,
                                    truncate_chars(m_text, 30),
                                    FontId::proportional(11.8),
                                    pal.primary_hover,
                                );
                                if r_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(m_text)
                                    .clicked()
                                {
                                    clicked_msg_hit = Some((*t_idx, query.clone()));
                                }
                            }
                        }

                        if !query.is_empty() && filtered_indices.is_empty() && msg_hits.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(32.0);
                                ui.label(
                                    RichText::new(
                                        "Không tìm thấy hội thoại hoặc tin nhắn phù hợp.",
                                    )
                                    .size(12.5)
                                    .color(pal.text_muted),
                                );
                            });
                        }
                    });

                if let Some((thread_id, is_group, value)) = preference_action {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::SetConversationPreference {
                            owner: self.user_uin.clone(),
                            thread_id,
                            is_group,
                            value,
                        });
                    }
                }
                if let Some(id) = read_action {
                    if let Some(thread) = self.threads.iter_mut().find(|t| t.id == id) {
                        thread.unread.read_visible(&thread.messages);
                    }
                    self.unread_counts.remove(&id);
                    self.persist_threads_async();
                    self.refresh_unread_title(ctx);
                }
                if let Some(id) = clear_history_action {
                    self.clear_history_confirmation = Some(id);
                }
                if let Some((id,remote)) = self.clear_history_confirmation.clone() {
                    let mut confirmed = false;
                    let mut cancelled = false;
                    egui::Window::new(if remote{"Xóa cuộc trò chuyện?"}else{"Xóa lịch sử trên máy này?"})
                        .id(Id::new("clear_thread_history"))
                        .collapsible(false)
                        .resizable(false)
                        .show(ctx, |ui| {
                            ui.label(
                                if remote{"Yêu cầu máy chủ xóa cuộc trò chuyện cho tài khoản này. Không thu hồi tin của người khác và không rời nhóm."}else{"Chỉ xóa bản lưu trên máy này, không thu hồi tin của người khác."},
                            );
                            ui.label(if remote{"Chỉ xóa dữ liệu trên máy sau khi máy chủ xác nhận. Hãy xuất lịch sử trước nếu cần giữ bản sao."}else{"Đồng bộ sau đó có thể tải lại lịch sử từ máy chủ."});
                            ui.horizontal(|ui| {
                                confirmed = ui.button("Xóa lịch sử").clicked();
                                cancelled = ui.button("Hủy").clicked();
                            });
                        });
                    if confirmed {
                        if let Some(tx)=WORKER_TX.get(){
                            if remote {if let Some(t)=self.threads.iter().find(|t|t.id==id){let _=tx.send(WorkerCmd::DeleteConversation{owner:self.user_uin.clone(),thread:Box::new(t.clone())});}}
                            else {let _=tx.send(WorkerCmd::ClearThreadHistory{owner:self.user_uin.clone(),thread_id:id.clone()});}
                        }
                    }
                    if confirmed || cancelled {
                        self.clear_history_confirmation = None;
                    }
                }
                if let Some((idx, q_str)) = clicked_msg_hit {
                    self.selected_thread_index = idx;
                    self.in_chat_search_open = true;
                    self.in_chat_search_focus_pending = true;
                    self.in_chat_search_query = q_str;
                }
                if let Some((idx, tid, is_group)) = clicked_thread {
                    self.open_thread(ctx, idx, tid, is_group);
                }
            });

        // Extract lightweight metadata for Header, Right Info Drawer, and Central Chat Stream
        let thread_meta = self.threads.get(self.selected_thread_index).map(|t| {
            (
                t.id.clone(),
                t.name.clone(),
                t.avatar.clone(),
                t.is_group,
                t.messages.len(),
            )
        });

        // -------------------------------------------------------------------
        // 3. COLLAPSIBLE RIGHT CONVERSATION INFO DRAWER (responsive width)
        // -------------------------------------------------------------------
        let show_info_drawer = self.info_drawer_open && !is_compact_w;
        let drawer_w = (screen_w * 0.26).clamp(240.0, 310.0);

        if show_info_drawer {
            if let Some((ref d_id, ref d_name, ref d_ava, d_is_group, d_msg_count)) = thread_meta {
                let shared_photos: Vec<(String, crate::message_view::Media, ChatMessage)> = self
                    .threads
                    .get(self.selected_thread_index)
                    .map(|t| {
                        t.messages
                            .iter()
                            .rev()
                            .filter_map(|m| {
                                let media = crate::message_view::media(m);
                                if !matches!(
                                    media.kind,
                                    crate::message_view::MediaKind::Photo
                                        | crate::message_view::MediaKind::Video
                                ) {
                                    return None;
                                }
                                let _thumb =
                                    media.thumbnail.as_deref().filter(|u| !u.is_empty())?;
                                Some((
                                    crate::media_recovery::image_key(&t.id, &m.id, 0, false),
                                    media,
                                    m.clone(),
                                ))
                            })
                            .take(9)
                            .collect()
                    })
                    .unwrap_or_default();

                egui::SidePanel::right("conversation_info_drawer")
                    .resizable(true)
                    .default_width(drawer_w)
                    .width_range(220.0..=340.0)
                    .frame(
                        egui::Frame::none()
                            .fill(pal.bg_sidebar)
                            .stroke(Stroke::new(1.0_f32, pal.border)),
                    )
                    .show(ctx, |ui| {
                        // Profile Card Header
                        egui::Frame::none()
                            .inner_margin(Margin::symmetric(16.0, 18.0))
                            .show(ui, |ui| {
                                ui.vertical_centered(|ui| {
                                    let (av_rect, _) = ui
                                        .allocate_exact_size(Vec2::new(64.0, 64.0), Sense::hover());
                                    paint_user_avatar(
                                        ui.painter(),
                                        av_rect.center(),
                                        32.0,
                                        d_name,
                                        d_is_group,
                                        d_ava.as_deref(),
                                        &self.sticker_textures,
                                        &mut self.loading_stickers,
                                    );
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(d_name)
                                            .size(14.5)
                                            .color(pal.text_primary)
                                            .strong(),
                                    );
                                    ui.add_space(2.0);
                                    let d_is_cloud = d_name.contains("Cloud của tôi");
                                    let d_fr_status = if d_is_group || d_is_cloud {
                                        crate::chat::FriendshipStatus::Friend
                                    } else {
                                        self.friend_status_map
                                            .get(d_id)
                                            .copied()
                                            .unwrap_or(crate::chat::FriendshipStatus::Unknown)
                                    };
                                    let (sub_txt, sub_col) = if d_is_group {
                                        ("Nhóm trò chuyện", pal.text_secondary)
                                    } else if d_is_cloud {
                                        ("Lưu trữ đám mây cá nhân", pal.primary_hover)
                                    } else {
                                        match d_fr_status {
                                            crate::chat::FriendshipStatus::Friend => {
                                                ("✓ Đã kết bạn", pal.text_secondary)
                                            }
                                            crate::chat::FriendshipStatus::IncomingPending => (
                                                "📩 Đã gửi lời mời kết bạn cho bạn",
                                                pal.primary_hover,
                                            ),
                                            crate::chat::FriendshipStatus::OutgoingPending => {
                                                ("⏳ Đã gửi lời mời kết bạn", pal.warning)
                                            }
                                            crate::chat::FriendshipStatus::Stranger => (
                                                "Người lạ · Chưa kết bạn",
                                                Color32::from_rgb(249, 115, 22),
                                            ),
                                            crate::chat::FriendshipStatus::Unknown => {
                                                ("Đang kiểm tra kết bạn…", pal.text_muted)
                                            }
                                        }
                                    };
                                    ui.label(RichText::new(sub_txt).size(12.0).color(sub_col));

                                    ui.add_space(14.0);
                                    // Quick Action Buttons (Sync & Pin/Star Filter)
                                    ui.horizontal(|ui| {
                                        ui.add_space(34.0);
                                        let sync_btn = egui::Button::new(
                                            RichText::new("Đồng bộ")
                                                .size(11.5)
                                                .color(pal.text_secondary),
                                        )
                                        .fill(pal.bg_elevated)
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .rounding(Rounding::same(8.0))
                                        .min_size(Vec2::new(92.0, 30.0));
                                        if ui
                                            .add(sync_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::RefreshThreads);
                                                if d_is_group || d_name.contains("Cloud của tôi")
                                                {
                                                    let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                                        limit: 500,
                                                        group_id: d_id.clone(),
                                                        is_group: d_is_group,
                                                    });
                                                }
                                            }
                                            self.show_toast("Đang đồng bộ lịch sử hội thoại…");
                                        }

                                        let star_active = self.filter_starred_only;
                                        let star_btn =
                                            egui::Button::new(
                                                RichText::new(if star_active {
                                                    "Hiện tất cả"
                                                } else {
                                                    "Tin đánh dấu"
                                                })
                                                .size(11.5)
                                                .color(if star_active {
                                                    Color32::WHITE
                                                } else {
                                                    pal.text_secondary
                                                }),
                                            )
                                            .fill(if star_active {
                                                pal.primary
                                            } else {
                                                pal.bg_elevated
                                            })
                                            .stroke(Stroke::new(1.0_f32, pal.border))
                                            .rounding(Rounding::same(8.0))
                                            .min_size(Vec2::new(92.0, 30.0));
                                        if ui
                                            .add(star_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.filter_starred_only = !self.filter_starred_only;
                                        }
                                    });
                                });
                            });

                        ui.add(egui::Separator::default().spacing(1.0));

                        // Shared Media Grid (`Ảnh / Video đã chia sẻ`)
                        egui::Frame::none()
                            .inner_margin(Margin::symmetric(16.0, 14.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Ảnh / Video đã chia sẻ")
                                            .size(12.5)
                                            .color(pal.text_primary)
                                            .strong(),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format!("({})", shared_photos.len()))
                                                .size(11.5)
                                                .color(pal.primary_hover),
                                        );
                                    });
                                });
                                ui.add_space(8.0);

                                if shared_photos.is_empty() {
                                    ui.label(
                                        RichText::new("Chưa có ảnh hoặc video chia sẻ.")
                                            .size(11.5)
                                            .color(pal.text_muted),
                                    );
                                } else {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                                        for (tkey, media, message) in &shared_photos {
                                            let (cell_r, cell_resp) = ui.allocate_exact_size(
                                                Vec2::splat(74.0),
                                                Sense::click(),
                                            );
                                            let thumb_u =
                                                media.thumbnail.as_deref().unwrap_or_default();
                                            let full_u =
                                                media.preview.as_deref().unwrap_or_default();
                                            let ptitle = &media.name;
                                            if ui.is_rect_visible(cell_r)
                                                && !self.sticker_textures.contains_key(tkey)
                                                && self.loading_stickers.insert(tkey.clone())
                                            {
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ = tx.send(WorkerCmd::FetchChatImage {
                                                        key: tkey.clone(),
                                                        url: thumb_u.to_owned(),
                                                        thread: d_id.clone(),
                                                        group: d_is_group,
                                                        message: Box::new(message.clone()),
                                                        preview: false,
                                                    });
                                                }
                                            }

                                            ui.painter().rect_filled(
                                                cell_r,
                                                Rounding::same(8.0),
                                                pal.bg_elevated,
                                            );
                                            if let Some(tex) = self.sticker_textures.get(tkey) {
                                                ui.painter().image(
                                                    tex.id(),
                                                    cell_r,
                                                    Rect::from_min_max(
                                                        Pos2::ZERO,
                                                        Pos2::new(1.0, 1.0),
                                                    ),
                                                    Color32::WHITE,
                                                );
                                            } else {
                                                draw_icon_image(
                                                    ui.painter(),
                                                    cell_r.center(),
                                                    20.0,
                                                    pal.text_muted,
                                                );
                                            }
                                            if media.kind==crate::message_view::MediaKind::Video {
                                                let c=cell_r.center();ui.painter().circle_filled(c,14.0,Color32::from_black_alpha(150));
                                                ui.painter().add(egui::Shape::convex_polygon(vec![c+Vec2::new(-4.0,-7.0),c+Vec2::new(8.0,0.0),c+Vec2::new(-4.0,7.0)],Color32::WHITE,Stroke::NONE));
                                            }
                                            ui.painter().rect_stroke(
                                                cell_r,
                                                Rounding::same(8.0),
                                                Stroke::new(
                                                    1.0_f32,
                                                    if cell_resp.hovered() {
                                                        pal.primary
                                                    } else {
                                                        pal.border
                                                    },
                                                ),
                                            );
                                            if cell_resp
                                                .on_hover_cursor(CursorIcon::PointingHand)
                                                .on_hover_text(
                                                    if media.kind
                                                        == crate::message_view::MediaKind::Video
                                                    {
                                                        "Phát video"
                                                    } else {
                                                        "Xem ảnh phóng to"
                                                    },
                                                )
                                                .clicked()
                                            {
                                                if media.kind
                                                    == crate::message_view::MediaKind::Video
                                                {
                                                    if let (Some(tx), Some(url)) =
                                                        (WORKER_TX.get(), media.original.clone())
                                                    {
                                                        let _ = tx.send(WorkerCmd::DownloadMedia {
                                                            url,
                                                            name: media.name.clone(),
                                                            play: true,
                                                            context: Some(
                                                                super::worker::ChatImage {
                                                                    thread: d_id.clone(),
                                                                    group: d_is_group,
                                                                    message: message.clone(),
                                                                    preview: false,
                                                                },
                                                            ),
                                                        });
                                                    }
                                                    continue;
                                                }
                                                let full_key = crate::media_recovery::image_key(
                                                    d_id,
                                                    &message.id,
                                                    0,
                                                    true,
                                                );
                                                if !full_u.is_empty()
                                                    && !self
                                                        .sticker_textures
                                                        .contains_key(&full_key)
                                                    && self
                                                        .loading_stickers
                                                        .insert(full_key.clone())
                                                {
                                                    if let Some(tx) = WORKER_TX.get() {
                                                        let _ =
                                                            tx.send(WorkerCmd::FetchChatImage {
                                                                key: full_key.clone(),
                                                                url: full_u.into(),
                                                                thread: d_id.clone(),
                                                                group: d_is_group,
                                                                message: Box::new(message.clone()),
                                                                preview: true,
                                                            });
                                                    }
                                                }
                                                self.image_viewer = Some(ImageViewerModal {
                                                    tex_key: tkey.clone(),
                                                    hd_tex_key: Some(full_key),
                                                    full_url: full_u.to_owned(),
                                                    original_url: media
                                                        .original
                                                        .clone()
                                                        .unwrap_or_default(),
                                                    title: ptitle.clone(),
                                                    sender: d_name.clone(),
                                                    time: String::new(),
                                                    zoom: 1.0,
                                                    pan: Vec2::ZERO,
                                                });
                                            }
                                        }
                                    });
                                }
                            });

                        // Push technical metadata to the bottom of the drawer
                        let rem = (ui.available_height() - 88.0).max(8.0);
                        ui.add_space(rem);
                        ui.add(egui::Separator::default().spacing(1.0));

                        egui::Frame::none()
                            .fill(pal.bg_app)
                            .inner_margin(Margin::symmetric(16.0, 12.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Tổng số tin nhắn:")
                                            .size(11.5)
                                            .color(pal.text_muted),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.label(
                                            RichText::new(format!("{d_msg_count} tin nhắn"))
                                                .size(11.5)
                                                .color(pal.text_secondary)
                                                .strong(),
                                        );
                                    });
                                });
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Mã định danh (ID):")
                                            .size(11.5)
                                            .color(pal.text_muted),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let short_id = if d_id.len() > 10 {
                                            format!(
                                                "{}...{}",
                                                &d_id[..4],
                                                &d_id[d_id.len().saturating_sub(4)..]
                                            )
                                        } else {
                                            d_id.clone()
                                        };
                                        let (id_rect, id_resp) = ui.allocate_exact_size(
                                            Vec2::new(98.0, 22.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            id_rect,
                                            Rounding::same(5.0),
                                            if id_resp.hovered() {
                                                pal.bg_hover
                                            } else {
                                                pal.bg_elevated
                                            },
                                        );
                                        ui.painter().text(
                                            Pos2::new(id_rect.left() + 7.0, id_rect.center().y),
                                            egui::Align2::LEFT_CENTER,
                                            short_id,
                                            FontId::monospace(10.5),
                                            pal.text_secondary,
                                        );
                                        draw_icon_copy(
                                            ui.painter(),
                                            Pos2::new(id_rect.right() - 10.0, id_rect.center().y),
                                            11.5,
                                            pal.text_secondary,
                                        );
                                        if id_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(format!("Sao chép ID: {d_id}"))
                                            .clicked()
                                        {
                                            ui.ctx().copy_text(d_id.clone());
                                            self.show_toast(
                                                "Đã sao chép ID hội thoại vào bộ nhớ tạm",
                                            );
                                        }
                                    });
                                });
                            });
                    });
            }
        }

        // -------------------------------------------------------------------
        // 4. CENTRAL CHAT AREA (Clean Header, Messages Stream, Icon Toolbar)
        // -------------------------------------------------------------------
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg_chat_canvas))
            .show(ctx, |ui| {
                if let Some((thread_id, thread_name, thread_avatar, is_group, _msg_count)) =
                    thread_meta
                {
                    let is_cloud_thread = thread_name.contains("Cloud của tôi");
                    if !is_group
                        && !is_cloud_thread
                        && !self.friend_status_map.contains_key(&thread_id)
                        && self.checking_friend_status.insert(thread_id.clone())
                    {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::CheckFriendStatus {
                                user_id: thread_id.clone(),
                            });
                        }
                    }
                    let cur_friendship = if is_group || is_cloud_thread {
                        crate::chat::FriendshipStatus::Friend
                    } else {
                        self.friend_status_map
                            .get(&thread_id)
                            .copied()
                            .unwrap_or(crate::chat::FriendshipStatus::Unknown)
                    };
                    // 1. Clean Conversation Header Bar (No raw 19-digit ID, no bulky sync button)
                    egui::Frame::none()
                        .fill(pal.bg_sidebar)
                        .inner_margin(Margin::symmetric(18.0, 11.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Header Avatar (40x40) with Cutout Online Dot
                                let (av_rect, _) =
                                    ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::hover());
                                paint_user_avatar(
                                    ui.painter(),
                                    av_rect.center(),
                                    20.0,
                                    &thread_name,
                                    is_group,
                                    thread_avatar.as_deref(),
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );
                                ui.add_space(6.0);
                                // Reserve room for the right-hand action cluster so the name/tag
                                // block can never be overlapped by the call buttons.
                                let actions_w = if is_compact_w { 126.0 } else { 182.0 };
                                let left_info_w =
                                    (ui.available_width() - actions_w).max(60.0);
                                ui.allocate_ui_with_layout(
                                    Vec2::new(left_info_w, 40.0),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.y = 2.0;
                                        ui.horizontal(|ui| {
                                            let name_max = if is_compact_w { 12 } else { 22 };
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(truncate_chars(
                                                        &thread_name,
                                                        name_max,
                                                    ))
                                                    .size(15.0)
                                                    .color(pal.text_primary)
                                                    .strong(),
                                                )
                                                .truncate(),
                                            );
                                            if !is_compact_w && left_info_w > 170.0 {
                                                let (tag_lbl, tag_fg, tag_bg) =
                                                    if is_cloud_thread {
                                                        (
                                                            "Đám mây",
                                                            pal.primary_hover,
                                                            pal.primary.linear_multiply(0.14),
                                                        )
                                                    } else if is_group {
                                                        ("Nhóm", pal.text_secondary, pal.bg_elevated)
                                                    } else {
                                                        match cur_friendship {
                                                            crate::chat::FriendshipStatus::Friend => (
                                                                "✓ Bạn bè",
                                                                pal.accent_online,
                                                                pal.accent_online.linear_multiply(0.14),
                                                            ),
                                                            crate::chat::FriendshipStatus::IncomingPending => (
                                                                "Chờ đồng ý kết bạn",
                                                                pal.primary_hover,
                                                                pal.primary.linear_multiply(0.16),
                                                            ),
                                                            crate::chat::FriendshipStatus::OutgoingPending => (
                                                                "Đã gửi lời mời",
                                                                pal.warning,
                                                                pal.warning.linear_multiply(0.15),
                                                            ),
                                                            crate::chat::FriendshipStatus::Stranger => (
                                                                "Người lạ",
                                                                Color32::from_rgb(249, 115, 22),
                                                                Color32::from_rgb(249, 115, 22)
                                                                    .linear_multiply(0.15),
                                                            ),
                                                            crate::chat::FriendshipStatus::Unknown => (
                                                                "Đang kiểm tra…",
                                                                pal.text_muted,
                                                                pal.bg_elevated,
                                                            ),
                                                        }
                                                    };
                                                egui::Frame::none()
                                                    .fill(tag_bg)
                                                    .rounding(Rounding::same(4.0))
                                                    .inner_margin(Margin::symmetric(6.0, 1.5))
                                                    .show(ui, |ui| {
                                                        ui.label(
                                                            RichText::new(tag_lbl)
                                                                .size(10.2)
                                                                .color(tag_fg)
                                                                .strong(),
                                                        );
                                                    });
                                            }
                                        });
                                        let (sub_status_lbl, sub_status_col) = if is_group {
                                            ("Nhóm trò chuyện", pal.text_secondary)
                                        } else if is_cloud_thread {
                                            ("Lưu trữ đám mây cá nhân", pal.text_secondary)
                                        } else {
                                            match cur_friendship {
                                                crate::chat::FriendshipStatus::Friend => {
                                                    ("Đã kết bạn", pal.text_secondary)
                                                }
                                                crate::chat::FriendshipStatus::IncomingPending => (
                                                    "Đã gửi lời mời kết bạn cho bạn",
                                                    pal.primary_hover,
                                                ),
                                                crate::chat::FriendshipStatus::OutgoingPending => (
                                                    "Đã gửi lời mời kết bạn · Chờ đồng ý",
                                                    pal.warning,
                                                ),
                                                crate::chat::FriendshipStatus::Stranger => (
                                                    "Người lạ · Chưa kết bạn trên Zalo",
                                                    Color32::from_rgb(249, 115, 22),
                                                ),
                                                crate::chat::FriendshipStatus::Unknown => {
                                                    ("Chưa có thông tin trạng thái", pal.text_secondary)
                                                }
                                            }
                                        };
                                        ui.label(
                                            RichText::new(sub_status_lbl)
                                                .size(11.6)
                                                .color(sub_status_col),
                                        );
                                    },
                                );

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    // Info Drawer Toggle Button (hidden on compact widths: auto-collapsed)
                                    if !is_compact_w {
                                        let (drw_rect, drw_resp) = ui.allocate_exact_size(
                                            Vec2::new(34.0, 34.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            drw_rect,
                                            Rounding::same(8.0),
                                            if self.info_drawer_open {
                                                pal.primary.linear_multiply(0.18)
                                            } else if drw_resp.hovered() {
                                                pal.bg_hover
                                            } else {
                                                Color32::TRANSPARENT
                                            },
                                        );
                                        draw_icon_drawer(
                                            ui.painter(),
                                            drw_rect.center(),
                                            17.0,
                                            if self.info_drawer_open {
                                                pal.primary_hover
                                            } else if drw_resp.hovered() {
                                                pal.text_primary
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                        if drw_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Thông tin hội thoại")
                                            .clicked()
                                        {
                                            self.info_drawer_open = !self.info_drawer_open;
                                            self.settings.info_drawer_open=self.info_drawer_open;
                                            if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::PersistInfoDrawer{owner:self.user_uin.clone(),open:self.info_drawer_open});}
                                        }
                                    }

                                    // Subtle vertical divider
                                    let (div_r, _) = ui.allocate_exact_size(
                                        Vec2::new(8.0, 20.0),
                                        Sense::hover(),
                                    );
                                    ui.painter().line_segment(
                                        [div_r.center_top(), div_r.center_bottom()],
                                        Stroke::new(1.0_f32, pal.border),
                                    );

                                    // Video Call Button
                                    let (vid_rect, vid_resp) = ui.allocate_exact_size(
                                        Vec2::new(34.0, 34.0),
                                        Sense::click(),
                                    );
                                    if vid_resp.hovered() {
                                        ui.painter().rect_filled(
                                            vid_rect,
                                            Rounding::same(8.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    draw_icon_video(
                                        ui.painter(),
                                        vid_rect.center(),
                                        17.5,
                                        if vid_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if vid_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text("Gọi Video")
                                        .clicked()
                                    {
                                        self.active_call = Some(ActiveCallOverlay {
                                            call_id: None,
                                            partner_id: thread_id.clone(),
                                            partner_name: thread_name.clone(),
                                            is_video: true,
                                            is_group,
                                            state_label: "Đang kết nối cuộc gọi video…".into(),
                                            started_at: std::time::Instant::now(),
                                            mic_muted: false,
                                            cam_enabled: true,
                                            is_incoming_ringing: false,
                                            error_code: None,
                                            error_reason: None,
                                        });
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::StartCall {
                                                partner_id: thread_id.clone(),
                                                partner_name: thread_name.clone(),
                                                partner_avatar: None,
                                                is_video: true,
                                                is_group,
                                            });
                                        }
                                    }

                                    // Voice Call Button
                                    let (aud_rect, aud_resp) = ui.allocate_exact_size(
                                        Vec2::new(34.0, 34.0),
                                        Sense::click(),
                                    );
                                    if aud_resp.hovered() {
                                        ui.painter().rect_filled(
                                            aud_rect,
                                            Rounding::same(8.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    draw_icon_phone(
                                        ui.painter(),
                                        aud_rect.center(),
                                        17.5,
                                        if aud_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if aud_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text("Gọi thoại")
                                        .clicked()
                                    {
                                        self.active_call = Some(ActiveCallOverlay {
                                            call_id: None,
                                            partner_id: thread_id.clone(),
                                            partner_name: thread_name.clone(),
                                            is_video: false,
                                            is_group,
                                            state_label: "Đang kết nối cuộc gọi thoại…".into(),
                                            started_at: std::time::Instant::now(),
                                            mic_muted: false,
                                            cam_enabled: false,
                                            is_incoming_ringing: false,
                                            error_code: None,
                                            error_reason: None,
                                        });
                                        if let Some(tx) = WORKER_TX.get() {
                                            let _ = tx.send(WorkerCmd::StartCall {
                                                partner_id: thread_id.clone(),
                                                partner_name: thread_name.clone(),
                                                partner_avatar: None,
                                                is_video: false,
                                                is_group,
                                            });
                                        }
                                    }
                                    // Search Messages in Conversation Button (`Ctrl+F`)
                                    let (srch_rect, srch_resp) = ui.allocate_exact_size(
                                        Vec2::new(34.0, 34.0),
                                        Sense::click(),
                                    );
                                    ui.painter().rect_filled(
                                        srch_rect,
                                        Rounding::same(8.0),
                                        if self.in_chat_search_open {
                                            pal.primary.linear_multiply(0.18)
                                        } else if srch_resp.hovered() {
                                            pal.bg_hover
                                        } else {
                                            Color32::TRANSPARENT
                                        },
                                    );
                                    draw_icon_search(
                                        ui.painter(),
                                        srch_rect.center(),
                                        16.5,
                                        if self.in_chat_search_open {
                                            pal.primary_hover
                                        } else if srch_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if srch_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text("Tìm kiếm tin nhắn trong hội thoại này (Ctrl+F)")
                                        .clicked()
                                        || ui.input(|i| {
                                            (i.modifiers.ctrl || i.modifiers.command)
                                                && i.key_pressed(Key::F)
                                        })
                                    {
                                        self.in_chat_search_open = !self.in_chat_search_open;
                                        if self.in_chat_search_open {
                                            self.in_chat_search_focus_pending = true;
                                        } else {
                                            self.in_chat_search_query.clear();
                                        }
                                    }
                                });
                            });
                        });
                    // 1a. Stranger / Pending Friend Request Banner (matching Zalo PC's top relationship bar)
                    if !is_group
                        && !is_cloud_thread
                        && matches!(
                            cur_friendship,
                            crate::chat::FriendshipStatus::Stranger
                                | crate::chat::FriendshipStatus::IncomingPending
                                | crate::chat::FriendshipStatus::OutgoingPending
                        )
                    {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(18.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    match cur_friendship {
                                        crate::chat::FriendshipStatus::IncomingPending => {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{}  {} đã gửi lời mời kết bạn cho bạn",
                                                    lucide::USER_PLUS,
                                                    truncate_chars(&thread_name, 26)
                                                ))
                                                .size(12.2)
                                                .color(pal.primary_hover)
                                                .strong(),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.spacing_mut().item_spacing.x = 6.0;
                                                    let rej_btn = egui::Button::new(
                                                        RichText::new("Từ chối")
                                                            .size(11.5)
                                                            .color(pal.text_secondary),
                                                    )
                                                    .fill(pal.bg_input)
                                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(68.0, 26.0));
                                                    if ui
                                                        .add(rej_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(
                                                                WorkerCmd::RejectFriend {
                                                                    user_id: thread_id.clone(),
                                                                    user_name: thread_name.clone(),
                                                                },
                                                            );
                                                        }
                                                    }

                                                    let acc_btn = egui::Button::new(
                                                        RichText::new(format!(
                                                            "{} Đồng ý kết bạn",
                                                            lucide::CHECK
                                                        ))
                                                        .size(11.8)
                                                        .color(Color32::WHITE)
                                                        .strong(),
                                                    )
                                                    .fill(pal.accent_online)
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(124.0, 26.0));
                                                    if ui
                                                        .add(acc_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(
                                                                WorkerCmd::AcceptFriend {
                                                                    user_id: thread_id.clone(),
                                                                    user_name: thread_name.clone(),
                                                                },
                                                            );
                                                        }
                                                    }
                                                },
                                            );
                                        }
                                        crate::chat::FriendshipStatus::OutgoingPending => {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Đã gửi lời mời kết bạn tới {} · Đang chờ đồng ý",
                                                    truncate_chars(&thread_name, 24)
                                                ))
                                                .size(12.0)
                                                .color(pal.warning),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    let undo_btn = egui::Button::new(
                                                        RichText::new("Hủy lời mời")
                                                            .size(11.5)
                                                            .color(pal.text_secondary),
                                                    )
                                                    .fill(pal.bg_input)
                                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(86.0, 26.0));
                                                    if ui
                                                        .add(undo_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(WorkerCmd::UndoFriend {
                                                                user_id: thread_id.clone(),
                                                                user_name: thread_name.clone(),
                                                            });
                                                        }
                                                    }
                                                },
                                            );
                                        }
                                        crate::chat::FriendshipStatus::Stranger => {
                                            ui.label(
                                                RichText::new(format!(
                                                    "Bạn và {} chưa là bạn bè trên Zalo",
                                                    truncate_chars(&thread_name, 26)
                                                ))
                                                .size(12.0)
                                                .color(pal.text_secondary),
                                            );
                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    let add_btn = egui::Button::new(
                                                        RichText::new(format!(
                                                            "{} Kết bạn",
                                                            lucide::USER_PLUS
                                                        ))
                                                        .size(11.8)
                                                        .color(Color32::WHITE)
                                                        .strong(),
                                                    )
                                                    .fill(pal.primary)
                                                    .rounding(Rounding::same(6.0))
                                                    .min_size(Vec2::new(92.0, 26.0));
                                                    if ui
                                                        .add(add_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(WorkerCmd::AddFriend {
                                                                user_id: thread_id.clone(),
                                                                user_name: thread_name.clone(),
                                                            });
                                                        }
                                                        self.show_toast(format!(
                                                            "Đã gửi lời mời kết bạn tới {}",
                                                            thread_name
                                                        ));
                                                    }
                                                },
                                            );
                                        }
                                        _ => {}
                                    }
                                });
                            });
                    }
                    // 1b. Pinned Message Banner (if any message is pinned in this thread)
                    let pinned_msg_info = self
                        .threads
                        .get(self.selected_thread_index)
                        .and_then(|t| {
                            t.messages
                                .iter()
                                .enumerate()
                                .find(|(_, m)| m.is_pinned)
                                .map(|(idx, m)| {
                                    (
                                        idx,
                                        if m.is_outgoing {
                                            "Tôi".to_string()
                                        } else if !m.sender.is_empty() && m.sender != "0" {
                                            m.sender.clone()
                                        } else {
                                            thread_name.clone()
                                        },
                                        if m.sticker_id.is_some() {
                                            "[Nhãn dán]".to_string()
                                        } else {
                                            truncate_chars(&m.text, 80).into_owned()
                                        },
                                    )
                                })
                        });

                    let mut unpin_idx: Option<usize> = None;
                    if let Some((pidx, psender, ptext)) = pinned_msg_info {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .stroke(Stroke::new(1.0_f32, pal.primary.linear_multiply(0.4)))
                            .inner_margin(Margin::symmetric(14.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{} Tin ghim:", lucide::PIN))
                                            .size(12.5)
                                            .color(pal.primary)
                                            .strong(),
                                    );
                                    ui.label(
                                        RichText::new(format!("{psender}: {ptext}"))
                                            .size(12.5)
                                            .color(pal.text_primary),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui
                                            .small_button(format!("Bỏ ghim {}", lucide::X))
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            unpin_idx = Some(pidx);
                                        }
                                    });
                                });
                            });
                    }
                    if let Some(uidx) = unpin_idx {
                        if let Some(real) = self
                            .threads
                            .get_mut(self.selected_thread_index)
                            .and_then(|t| t.messages.get_mut(uidx))
                        {
                            real.is_pinned = false;
                        }
                        self.persist_threads_async();
                        self.status_msg = "Đã bỏ ghim tin nhắn".into();
                    }

                    // 1c. Starred filter active banner
                    if self.filter_starred_only {
                        egui::Frame::none()
                            .fill(Color32::from_rgb(234, 179, 8).linear_multiply(0.12))
                            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(234, 179, 8)))
                            .inner_margin(Margin::symmetric(14.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("{} Đang lọc tin nhắn đã đánh dấu sao", lucide::STAR))
                                            .size(12.0)
                                            .color(Color32::from_rgb(234, 179, 8))
                                            .strong(),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui
                                            .small_button(format!("Xem tất cả {}", lucide::X))
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked()
                                        {
                                            self.filter_starred_only = false;
                                        }
                                    });
                                });
                            });
                    }

                    // 1d. In-Conversation Message Search Bar (`self.in_chat_search_open`)
                    if self.in_chat_search_open
                        && ui.input(|i| i.key_pressed(Key::Escape)) {
                            self.in_chat_search_open = false;
                            self.in_chat_search_query.clear();
                        }
                    if self.in_chat_search_open {
                        let q_low = self.in_chat_search_query.trim().to_lowercase();
                        let q_folded = fold_vn_search(&q_low);
                        let match_cnt = if q_low.is_empty() {
                            0
                        } else {
                            self.threads
                                .get(self.selected_thread_index)
                                .map(|t| {
                                    t.messages
                                        .iter()
                                        .filter(|m| {
                                            matches_vn_query(&m.text, &q_low, &q_folded)
                                                || matches_vn_query(&m.sender, &q_low, &q_folded)
                                        })
                                        .count()
                                })
                                .unwrap_or(0)
                        };
                        egui::Frame::none()
                            .fill(pal.bg_sidebar)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(18.0, 8.0))
                            .show(ui, |ui| {
                                let bar_avail_w = ui.available_width();
                                ui.set_width(bar_avail_w);
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    let right_reserve = if q_low.is_empty() {
                                        86.0
                                    } else {
                                        174.0
                                    };
                                    let pill_w = (bar_avail_w - right_reserve).max(140.0);
                                    egui::Frame::none()
                                        .fill(pal.bg_input)
                                        .rounding(Rounding::same(8.0))
                                        .stroke(Stroke::new(
                                            1.0_f32,
                                            pal.primary.linear_multiply(0.65),
                                        ))
                                        .inner_margin(Margin::symmetric(10.0, 5.0))
                                        .show(ui, |ui| {
                                            let inner_w = (pill_w - 20.0).max(110.0);
                                            ui.set_width(inner_w);
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 6.0;
                                                let (ir, _) = ui.allocate_exact_size(
                                                    Vec2::new(15.0, 18.0),
                                                    Sense::hover(),
                                                );
                                                draw_icon_search(
                                                    ui.painter(),
                                                    ir.center(),
                                                    13.5,
                                                    pal.primary_hover,
                                                );
                                                let has_txt =
                                                    !self.in_chat_search_query.is_empty();
                                                let te_w = if has_txt {
                                                    (inner_w - 44.0).max(60.0)
                                                } else {
                                                    (inner_w - 22.0).max(80.0)
                                                };
                                                let te_resp = ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.in_chat_search_query,
                                                    )
                                                    .id(Id::new((
                                                        "in_chat_search_input",
                                                        &thread_id,
                                                    )))
                                                    .frame(false)
                                                    .hint_text(
                                                        "Tìm tin nhắn trong hội thoại này...",
                                                    )
                                                    .desired_width(te_w),
                                                );
                                                if self.in_chat_search_focus_pending {
                                                    te_resp.request_focus();
                                                    self.in_chat_search_focus_pending = false;
                                                }
                                                if has_txt {
                                                    let (clr_r, clr_resp) = ui
                                                        .allocate_exact_size(
                                                            Vec2::splat(16.0),
                                                            Sense::click(),
                                                        );
                                                    if clr_resp.hovered() {
                                                        ui.painter().circle_filled(
                                                            clr_r.center(),
                                                            8.0,
                                                            pal.bg_hover,
                                                        );
                                                    }
                                                    draw_lucide_icon(
                                                        ui.painter(),
                                                        clr_r.center(),
                                                        lucide::X,
                                                        11.5,
                                                        pal.text_muted,
                                                    );
                                                    if clr_resp
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .on_hover_text("Xóa từ khóa")
                                                        .clicked()
                                                    {
                                                        self.in_chat_search_query.clear();
                                                    }
                                                }
                                            });
                                        });

                                    if !q_low.is_empty() {
                                        let badge_col = if match_cnt > 0 {
                                            pal.accent_online
                                        } else {
                                            pal.danger
                                        };
                                        egui::Frame::none()
                                            .fill(badge_col.linear_multiply(0.14))
                                            .rounding(Rounding::same(6.0))
                                            .inner_margin(Margin::symmetric(8.0, 4.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(if match_cnt > 0 {
                                                        format!("{match_cnt} kết quả")
                                                    } else {
                                                        "0 kết quả".to_string()
                                                    })
                                                    .size(11.5)
                                                    .color(badge_col)
                                                    .strong(),
                                                );
                                            });
                                    }

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let close_btn = egui::Button::new(
                                            RichText::new(format!("{} Đóng", lucide::X))
                                                .size(11.8)
                                                .color(pal.text_secondary)
                                                .strong(),
                                        )
                                        .fill(pal.bg_elevated)
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .rounding(Rounding::same(6.0))
                                        .min_size(Vec2::new(66.0, 28.0));
                                        if ui
                                            .add(close_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Đóng thanh tìm kiếm (Esc)")
                                            .clicked()
                                        {
                                            self.in_chat_search_open = false;
                                            self.in_chat_search_query.clear();
                                        }
                                    });
                                });
                            });
                    }

                    // 2. Scrollable Messages Area
                    let contextual_stickers = self.resolve_contextual_sticker_suggestions(ctx);
                    let composer_link_preview = self.resolve_composer_link_preview(ctx);
                    // Do NOT subtract `contextual_stickers` height from `picker_reserve`!
                    // The sticker strip is a true floating overlay (`egui::Area`) above the composer bar
                    // (matching Zalo PC's `PopoverIdentitiesDefine.STICKER_SUGGESTION`), so the message scroll
                    // area and composer input box never shift or lose IME focus while typing.
                    let picker_reserve = (if self.sticker_picker_open { 268.0 } else { 0.0 })
                        + (if self.replying_to.is_some() { 48.0 } else { 0.0 })
                        + (if self.pending_clipboard_image.is_some() {
                            64.0
                        } else {
                            0.0
                        });
                    let mut pending_action: Option<(usize, BubbleAction)> = None;
                    let mut load_older_clicked = false;
                    if self.chat_render_window.0 != thread_id {
                        self.chat_render_window = (thread_id.clone(), 150);
                    }
                    let jump_index = self.quote_jump.as_ref().and_then(|jump| {
                        self.threads
                            .get(self.selected_thread_index)
                            .and_then(|thread| jump.target_index(&thread_id, &thread.messages))
                    });
                    if jump_index.is_none() {
                        self.quote_jump = None;
                    }
                    let jump_scroll_pending = self
                        .quote_jump
                        .as_ref()
                        .map(|jump| jump.scroll_pending)
                        .unwrap_or(false);
                    let mut jump_scrolled = false;
                    if let Some(index) = jump_index {
                        let count = self
                            .threads
                            .get(self.selected_thread_index)
                            .map(|thread| thread.messages.len())
                            .unwrap_or(0);
                        self.chat_render_window.1 = crate::message_navigation::render_limit_for_target(
                            self.chat_render_window.1,
                            count,
                            index,
                        );
                    }
                    let render_limit = self.chat_render_window.1;
                    // Measure visual wrapping, not only explicit newlines: the info drawer
                    // can leave a narrow editor even on a wide desktop window.
                    let composer_compact=(ui.available_width()-44.0)<360.0;
                    let composer_send_width=if composer_compact{52.0}else{76.0};
                    let composer_schedule_width=if composer_compact{0.0}else{38.0};
                    let composer_action_count=if ui.available_width()<230.0{0}else if ui.available_width()<300.0{1}else{3};
                    let composer_spacing=match composer_action_count {0=>16.0,1=>24.0,_=>36.0};
                    let composer_separator=if composer_action_count>0{8.0}else{0.0};
                    let composer_input_width=(ui.available_width()-44.0-34.0*composer_action_count as f32-composer_separator-composer_send_width-composer_schedule_width-composer_spacing).max(32.0);
                    self.sync_composer_draft(ctx);
                    let editor_height=composer_editor_height(ui,&self.input_message,composer_input_width);
                    let msg_area_height =
                        (ui.available_height() - 48.0 - picker_reserve - editor_height).max(0.0);

                    egui::ScrollArea::vertical()
                        .id_salt(Id::new(("chat_scroll", &thread_id)))
                        .min_scrolled_height(0.0)
                        .max_height(msg_area_height)
                        .auto_shrink([false, false])
                        .stick_to_bottom(jump_index.is_none())
                        .show(ui, |ui| {
                            egui::Frame::none()
                                .inner_margin(Margin::symmetric(18.0, 12.0))
                                .show(ui, |ui| {
                                    if let Some(thread) =
                                        self.threads.get(self.selected_thread_index)
                                    {
                                        if thread.messages.is_empty() {
                                            ui.vertical_centered(|ui| {
                                                ui.add_space(60.0);
                                                ui.label(
                                                    RichText::new(
                                                        "Chưa có tin nhắn trong cuộc trò chuyện này.",
                                                    )
                                                    .size(14.0)
                                                    .color(pal.text_muted),
                                                );
                                            });
                                        } else {
                                            if !self.in_chat_search_open
                                                && !self.filter_starred_only
                                            {
                                                ui.vertical_centered(|ui| {
                                                    let hist_btn = egui::Button::new(
                                                        RichText::new(format!(
                                                            "{} Tải thêm tin nhắn cũ",
                                                            lucide::REFRESH
                                                        ))
                                                        .size(11.5)
                                                        .color(pal.text_secondary),
                                                    )
                                                    .fill(pal.bg_elevated)
                                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                                    .rounding(Rounding::same(12.0));
                                                    if ui
                                                        .add(hist_btn)
                                                        .on_hover_cursor(CursorIcon::PointingHand)
                                                        .on_hover_text(
                                                            "Đồng bộ thêm lịch sử tin nhắn cũ từ máy chủ Zalo",
                                                        )
                                                        .clicked()
                                                    {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(
                                                                WorkerCmd::LoadGroupHistory {
                                                                    limit: render_limit.saturating_add(150).max(500),
                                                                    group_id: thread_id.clone(),
                                                                    is_group,
                                                                },
                                                            );
                                                        }
                                                        load_older_clicked = true;
                                                    }
                                                    ui.add_space(6.0);
                                                });
                                            }
                                            let only_starred = self.filter_starred_only;
                                            let in_chat_q = if self.in_chat_search_open {
                                                self.in_chat_search_query.trim().to_lowercase()
                                            } else {
                                                String::new()
                                            };
                                            let in_chat_folded = fold_vn_search(&in_chat_q);
                                            let avatar_by_sender:std::collections::HashMap<String,String>=self.threads.iter().filter(|t|!t.is_group).filter_map(|t|t.avatar.as_ref().map(|a|(t,a))).flat_map(|(t,a)|[(t.id.clone(),a.clone()),(t.presentation.global_id.clone(),a.clone())]).filter(|(id,_)|!id.is_empty()).collect();
                                            let visible_indices: Vec<usize> = thread
                                                .messages
                                                .iter()
                                                .enumerate()
                                                .filter(|(_, m)| {
                                                    if crate::message_view::is_call_log(m){return false;}
                                                    (!only_starred || m.is_starred)
                                                        && (in_chat_q.is_empty()
                                                            || matches_vn_query(
                                                                &m.text,
                                                                &in_chat_q,
                                                                &in_chat_folded,
                                                            )
                                                            || matches_vn_query(
                                                                &m.sender,
                                                                &in_chat_q,
                                                                &in_chat_folded,
                                                            ))
                                                })
                                                .map(|(i, _)| i)
                                                .collect();
                                            let visible_indices = if only_starred || !in_chat_q.is_empty() {
                                                visible_indices
                                            } else {
                                                let skip = visible_indices.len().saturating_sub(render_limit);
                                                visible_indices[skip..].to_vec()
                                            };
                                            if visible_indices.is_empty() && !in_chat_q.is_empty() {
                                                ui.vertical_centered(|ui| {
                                                    ui.add_space(56.0);
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "Không tìm thấy tin nhắn nào khớp từ khóa \"{}\".",
                                                            self.in_chat_search_query.trim()
                                                        ))
                                                        .size(13.5)
                                                        .color(pal.text_muted),
                                                    );
                                                });
                                            }
                                            let last_outgoing_idx = thread
                                                .messages
                                                .iter()
                                                .enumerate()
                                                .rev()
                                                .find(|(_, m)| {
                                                    m.is_outgoing && !is_system_notice_message(m)
                                                })
                                                .map(|(i, _)| i);

                                            for (pos, &idx) in visible_indices.iter().enumerate() {
                                                let msg = &thread.messages[idx];
                                                let prev_msg = pos
                                                    .checked_sub(1)
                                                    .and_then(|p| visible_indices.get(p))
                                                    .map(|&pi| &thread.messages[pi]);
                                                let next_msg = visible_indices
                                                    .get(pos + 1)
                                                    .map(|&ni| &thread.messages[ni]);

                                                if should_show_time_divider(prev_msg, msg) {
                                                    if pos > 0 {
                                                        ui.add_space(6.0);
                                                    }
                                                    render_date_divider(ui, msg, pal);
                                                    ui.add_space(6.0);
                                                }

                                                let row = ui.scope(|ui| {
                                                    if is_system_notice_message(msg) {
                                                        render_system_notice_pill(ui, msg, pal);
                                                        ui.add_space(6.0);
                                                        return;
                                                    }

                                                    let prev_same_sender = prev_msg
                                                        .map(|pm| same_message_cluster(pm, msg))
                                                        .unwrap_or(false);
                                                    let next_same_sender = next_msg
                                                        .map(|nm| same_message_cluster(msg, nm))
                                                        .unwrap_or(false);
                                                    let is_latest_outgoing =
                                                        last_outgoing_idx == Some(idx);

                                                    let sender_ava_opt = if !is_group {
                                                        thread_avatar.as_deref()
                                                    } else {
                                                        msg.wire_ref.sender.as_ref().and_then(|v|v.as_str().map(str::to_owned).or_else(||v.as_u64().map(|n|n.to_string()))).and_then(|id|avatar_by_sender.get(&id)).map(String::as_str)
                                                    };
                                                    if let Some(action) = render_chat_bubble(
                                                        ui,
                                                        msg,
                                                        prev_same_sender,
                                                        next_same_sender,
                                                        is_latest_outgoing,
                                                        is_group,
                                                        &thread_name,
                                                        &self.threads[self.selected_thread_index].id,
                                                        &self.image_failures,
                                                        sender_ava_opt,
                                                        pal,
                                                        &self.sticker_textures,
                                                        &mut self.loading_stickers,
                                                        &self.emoji_textures,
                                                        idx,
                                                    ) {
                                                        pending_action = Some((idx, action));
                                                    }
                                                    ui.add_space(if next_same_sender {
                                                        2.5
                                                    } else {
                                                        9.0
                                                    });
                                                });
                                                if jump_index == Some(idx) {
                                                    render_quote_target(
                                                        ui,
                                                        row.response.rect,
                                                        jump_scroll_pending,
                                                        pal.primary,
                                                    );
                                                    jump_scrolled |= jump_scroll_pending;
                                                }
                                            }
                                        }
                                    }
                                });
                        });
                    if jump_scrolled {
                        if let Some(jump) = &mut self.quote_jump {
                            jump.scroll_pending = false;
                        }
                    }
                    if load_older_clicked {
                        self.chat_render_window.1 = self.chat_render_window.1.saturating_add(150);
                        self.show_toast("Đang tải thêm tin nhắn cũ từ máy chủ Zalo…");
                    }

                    if let Some((idx, action)) = pending_action {
                        let target = self
                            .threads
                            .get(self.selected_thread_index)
                            .and_then(|t| t.messages.get(idx))
                            .cloned();
                        if let Some(m) = target {
                            match action {
                                BubbleAction::RetryMedia{key}=>{let preview=crate::media_recovery::image_key(&self.threads[self.selected_thread_index].id,&m.id,0,true);for key in [key,preview]{self.loading_stickers.remove(&key);self.image_failures.remove(&key);self.image_retry_at.remove(&key);}}
                                BubbleAction::MediaUnavailable=>{self.status_msg="Thiếu thông tin tải ảnh/video; chưa thể mở tệp".into();}

                                BubbleAction::SaveAttachment {url,name} => {
                                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::DownloadMedia{url,name,play:false,context:Some(super::worker::ChatImage{thread:self.threads[self.selected_thread_index].id.clone(),group:self.threads[self.selected_thread_index].is_group,message:m.clone(),preview:false})});}
                                }
                                BubbleAction::OpenVideo {url,name} => {
                                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::DownloadMedia{url,name,play:true,context:Some(super::worker::ChatImage{thread:self.threads[self.selected_thread_index].id.clone(),group:self.threads[self.selected_thread_index].is_group,message:m.clone(),preview:false})});}
                                }
                                BubbleAction::RetrySend => {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let thread_id = self.threads[self.selected_thread_index].id.clone();
                                        let mut message = m.clone();
                                        message.delivery = crate::message_model::DeliveryState::Pending;
                                        if let Some(job) = message.outgoing_job.as_mut() { job.attempt += 1; }
                                        self.threads[self.selected_thread_index].messages[idx] = message.clone();
                                        let _ = tx.send(WorkerCmd::RetryOutgoing { thread_id, message });
                                    }
                                }
                                BubbleAction::Reply => self.replying_to = Some(m),
                                BubbleAction::DoubleClickReply => { if self.settings.double_click_to_reply {self.replying_to=Some(m);} },
                                BubbleAction::JumpToQuoted(reference) => {
                                    let index = self
                                        .threads
                                        .get(self.selected_thread_index)
                                        .and_then(|thread| {
                                            crate::message_navigation::unique_reference_index(&thread.messages, &reference)
                                        });
                                    if index.is_some() {
                                        self.filter_starred_only = false;
                                        self.in_chat_search_open = false;
                                        self.in_chat_search_query.clear();
                                        self.quote_jump = Some(QuoteJump {
                                            thread_id: thread_id.clone(),
                                            reference,
                                            highlight_until: std::time::Instant::now() + std::time::Duration::from_secs(3),
                                            scroll_pending: true,
                                        });
                                        ctx.request_repaint();
                                    } else {
                                        self.show_toast("Chưa có tin nhắn gốc đã tải với ID duy nhất. Hãy tải thêm lịch sử rồi thử lại.");
                                    }
                                }
                                BubbleAction::Forward => {
                                    self.forward_msg = Some(m);
                                    self.forward_filter.clear();
                                }
                                BubbleAction::React(r) => {
                                    if let Some(real) = self
                                        .threads
                                        .get_mut(self.selected_thread_index)
                                        .and_then(|t| t.messages.get_mut(idx))
                                    {
                                        if real.reactions.contains(&r.to_string()) {
                                            real.reactions.clear();
                                            self.status_msg = "Đã bỏ cảm xúc".into();
                                        } else {
                                            real.reactions = vec![r.to_string()];
                                            self.status_msg = format!("Đã thả cảm xúc {r}");
                                        }
                                    }
                                    self.persist_threads_async();
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::React {
                                            to_id: thread_id.clone(),
                                            is_group,
                                            msg: m,
                                            reaction: r.to_string(),
                                        });
                                    }
                                }
                                BubbleAction::TogglePin | BubbleAction::ToggleStar => {
                                    if let Some(real) = self
                                        .threads
                                        .get_mut(self.selected_thread_index)
                                        .and_then(|t| t.messages.get_mut(idx))
                                    {
                                        if action == BubbleAction::TogglePin {
                                            real.is_pinned = !real.is_pinned;
                                            self.status_msg = if real.is_pinned {
                                                "Đã ghim tin nhắn lên đầu hội thoại".into()
                                            } else {
                                                "Đã bỏ ghim tin nhắn".into()
                                            };
                                        } else {
                                            real.is_starred = !real.is_starred;
                                            self.status_msg = if real.is_starred {
                                                "Đã đánh dấu sao tin nhắn".into()
                                            } else {
                                                "Đã bỏ đánh dấu sao".into()
                                            };
                                        }
                                    }
                                    self.persist_threads_async();
                                }
                                BubbleAction::CallAgain { is_video } => {
                                    self.active_call = Some(ActiveCallOverlay {
                                            call_id: None,
                                        partner_id: thread_id.clone(),
                                        partner_name: thread_name.clone(),
                                        is_video,
                                        is_group,
                                        state_label: if is_video {
                                            "Đang kết nối cuộc gọi video…".into()
                                        } else {
                                            "Đang kết nối cuộc gọi thoại…".into()
                                        },
                                        started_at: std::time::Instant::now(),
                                        mic_muted: false,
                                        cam_enabled: is_video,
                                        is_incoming_ringing: false,
                                        error_code: None,
                                        error_reason: None,
                                    });
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::StartCall {
                                            partner_id: thread_id.clone(),
                                            partner_name: thread_name.clone(),
                                            partner_avatar: None,
                                            is_video,
                                            is_group,
                                        });
                                    }
                                }
                                BubbleAction::DeleteForMe => {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::DeleteForMe {
                                            to_id: thread_id.clone(),
                                            is_group,
                                            msg: m,
                                        });
                                    }
                                }
                                BubbleAction::RecallForEveryone => {
                                    if let Some(real) = self
                                        .threads
                                        .get_mut(self.selected_thread_index)
                                        .and_then(|t| t.messages.get_mut(idx))
                                    {
                                        real.text = "[Tin nhắn đã thu hồi]".into();
                                        real.sticker_id = None;
                                        real.link_url = None;
                                        real.link_thumb = None;
                                    }
                                    self.persist_threads_async();
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::RecallMessage {
                                            to_id: thread_id.clone(),
                                            is_group,
                                            msg: m,
                                        });
                                    }
                                }
                                BubbleAction::OpenImagePopup {
                                    tex_key,
                                    full_url,
                                    title,
                                    sender,
                                    time,
                                } => {
                                    let hd_tex_key = if !full_url.is_empty() {
                                        let hk = crate::media_recovery::image_key(&self.threads[self.selected_thread_index].id,&m.id,0,true);
                                        if hk != tex_key
                                            && !self.sticker_textures.contains_key(&hk)
                                            && self.loading_stickers.insert(hk.clone())
                                        {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::FetchChatImage {
                                                    key: hk.clone(),url:full_url.clone(),thread:self.threads[self.selected_thread_index].id.clone(),group:self.threads[self.selected_thread_index].is_group,message:Box::new(m.clone()),preview:true
                                                });
                                            }
                                        }
                                        Some(hk)
                                    } else {
                                        None
                                    };
                                    self.image_viewer = Some(ImageViewerModal {
                                        tex_key,
                                        hd_tex_key,
                                        original_url: crate::message_view::media(&m).original.unwrap_or_else(||full_url.clone()),
                                        full_url,
                                        title,
                                        sender,
                                        time,
                                        zoom: 1.0,
                                        pan: Vec2::ZERO,
                                    });
                                }
                            }
                        }
                    }

                    // 3. Optional Sticker Picker Drawer
                    if self.sticker_picker_open {
                        self.render_sticker_picker(ui, &thread_id, is_group, pal);
                    }

                    // 3b. Reply banner (quoted message preview above the composer)
                    if self.replying_to.is_some() {
                        let mut cancel = false;
                        if let Some(q) = &self.replying_to {
                            let who = if q.is_outgoing {
                                "chính bạn".to_string()
                            } else if !q.sender.is_empty() && q.sender != "0" {
                                q.sender.clone()
                            } else {
                                thread_name.clone()
                            };
                            egui::Frame::none()
                                .fill(pal.bg_elevated)
                                .inner_margin(Margin::symmetric(14.0, 6.0))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!("Đang trả lời {who}"))
                                                    .size(12.0)
                                                    .color(pal.primary)
                                                    .strong(),
                                            );
                                            let preview = if q.sticker_id.is_some() {
                                                "[Nhãn dán]".to_string()
                                            } else {
                                                truncate_chars(&q.text, 100).into_owned()
                                            };
                                            ui.label(
                                                RichText::new(preview)
                                                    .size(12.0)
                                                    .color(pal.text_muted),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button(lucide::X)
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("Hủy trả lời")
                                                    .clicked()
                                                {
                                                    cancel = true;
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                        if cancel {
                            self.replying_to = None;
                        }
                    }
                    // 3c. Clipboard Image Preview Banner (`Ctrl+V` / `Shift+Insert` screenshot or image file paste)
                    if is_clipboard_paste_triggered(ctx) && clipboard_has_image() {
                        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
                        self.request_clipboard_image();
                    }
                    if self.pending_clipboard_image.is_some() {
                        let mut clear_clip = false;
                        if ui.input(|i| i.key_pressed(Key::Escape)) {
                            clear_clip = true;
                        }
                        if let Some((ref path, w, h, ref tex)) = self.pending_clipboard_image {
                            egui::Frame::none()
                                .fill(pal.bg_elevated)
                                .inner_margin(Margin::symmetric(14.0, 6.0))
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    pal.primary.linear_multiply(0.55),
                                ))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::Image::new(tex)
                                                .fit_to_exact_size(Vec2::new(48.0, 48.0))
                                                .rounding(Rounding::same(6.0)),
                                        );
                                        ui.add_space(6.0);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!(
                                                    "{} Ảnh chụp từ bộ nhớ tạm (Ctrl+V)",
                                                    lucide::IMAGE
                                                ))
                                                .size(12.2)
                                                .color(pal.primary)
                                                .strong(),
                                            );
                                            let fname = path
                                                .file_name()
                                                .and_then(|n| n.to_str())
                                                .unwrap_or("clipboard.png");
                                            ui.label(
                                                RichText::new(format!(
                                                    "{fname} · {w}×{h} px — Nhập chú thích bên dưới rồi nhấn Enter để gửi"
                                                ))
                                                .size(11.3)
                                                .color(pal.text_secondary),
                                            );
                                        });
                                        ui.with_layout(
                                            Layout::right_to_left(Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button(format!("{} Hủy", lucide::X))
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text("Hủy ảnh dán (Esc)")
                                                    .clicked()
                                                {
                                                    clear_clip = true;
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                        if clear_clip {
                            self.pending_clipboard_image = None;
                        }
                    }

                    // 4. Bottom Message Composer Bar (Compact Icon Actions + Spacious Input + Padded Send Button)
                    let composer_frame = egui::Frame::none()
                        .fill(pal.bg_sidebar)
                        .inner_margin(Margin::symmetric(14.0, 10.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .show(ui, |ui| {
                            // Handle drag-and-drop files onto the chat window
                            let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| {
                                i.raw
                                    .dropped_files
                                    .iter()
                                    .filter_map(|df| df.path.clone())
                                    .collect()
                            });
                            for dpath in dropped {
                                if let Some(tx) = WORKER_TX.get() {
                                    let _ = tx.send(WorkerCmd::PickAndSendAttachment {
                                        to_id: thread_id.clone(),
                                        is_group,
                                        images_only: false,
                                        caption: String::new(),
                                        preset_path: Some(dpath),
                                    });
                                }
                            }

                            egui::Frame::none()
                                .fill(pal.bg_input)
                                .rounding(Rounding::same(11.0))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .inner_margin(Margin::symmetric(8.0, 5.0))
                                .show(ui, |ui| {
                                    ui.horizontal_top(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;

                                        if composer_action_count==3 {
                                        // Sticker and photo actions move into the send menu on narrow views.
                                        let (stk_rect, stk_resp) = ui.allocate_exact_size(
                                            Vec2::new(34.0, 34.0),
                                            Sense::click(),
                                        );
                                        if self.sticker_picker_open || stk_resp.hovered() {
                                            ui.painter().rect_filled(
                                                stk_rect,
                                                Rounding::same(8.0),
                                                pal.bg_hover,
                                            );
                                        }
                                        draw_icon_sticker(
                                            ui.painter(),
                                            stk_rect.center(),
                                            18.0,
                                            if self.sticker_picker_open || stk_resp.hovered() {
                                                Color32::from_rgb(251, 191, 36)
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                        if stk_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Nhãn dán (Sticker)")
                                            .clicked()
                                        {
                                            self.sticker_picker_open = !self.sticker_picker_open;
                                        }

                                        // 2) Compact Photo Upload Icon Button
                                        let (img_rect, img_resp) = ui.allocate_exact_size(
                                            Vec2::new(34.0, 34.0),
                                            Sense::click(),
                                        );
                                        if img_resp.hovered() {
                                            ui.painter().rect_filled(
                                                img_rect,
                                                Rounding::same(8.0),
                                                pal.bg_hover,
                                            );
                                        }
                                        draw_icon_image(
                                            ui.painter(),
                                            img_rect.center(),
                                            18.0,
                                            if img_resp.hovered() {
                                                pal.accent_online
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                        if img_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Gửi hình ảnh")
                                            .clicked()
                                        {
                                            let cap = self.input_message.trim().to_string();
                                            self.input_message.clear();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::PickAndSendAttachment {
                                                    to_id: thread_id.clone(),
                                                    is_group,
                                                    images_only: true,
                                                    caption: cap,
                                                    preset_path: None,
                                                });
                                            }
                                        }

                                        }
                                        if composer_action_count>0 {
                                        // Keep a single attachment action when the full toolbar cannot fit.
                                        let (att_rect, att_resp) = ui.allocate_exact_size(
                                            Vec2::new(34.0, 34.0),
                                            Sense::click(),
                                        );
                                        if att_resp.hovered() {
                                            ui.painter().rect_filled(
                                                att_rect,
                                                Rounding::same(8.0),
                                                pal.bg_hover,
                                            );
                                        }
                                        draw_icon_attachment(
                                            ui.painter(),
                                            att_rect.center(),
                                            17.0,
                                            if att_resp.hovered() {
                                                pal.primary_hover
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                        if att_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text("Đính kèm File / Video")
                                            .clicked()
                                        {
                                            let cap = self.input_message.trim().to_string();
                                            self.input_message.clear();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::PickAndSendAttachment {
                                                    to_id: thread_id.clone(),
                                                    is_group,
                                                    images_only: false,
                                                    caption: cap,
                                                    preset_path: None,
                                                });
                                            }
                                        }

                                        // Vertical hairline separator
                                        let (sep_r, _) = ui.allocate_exact_size(
                                            Vec2::new(8.0, 20.0),
                                            Sense::hover(),
                                        );
                                        ui.painter().line_segment(
                                            [sep_r.center_top(), sep_r.center_bottom()],
                                            Stroke::new(1.0_f32, pal.border),
                                        );
                                        }
                                        // Keep a usable editor and send action even at high DPI.
                                        let send_btn_w = composer_send_width;
                                        let input_w = composer_input_width;
                                        let composer_id = Id::new(("chat_composer_input", &thread_id));
                                        // Enter sends; Shift+Enter is left to the editor as a newline.
                                        // Only consumed while the composer owns focus, so other fields keep Enter.
                                        let enter_send = self.quick_schedule.is_none() && ui.memory(|m| m.has_focus(composer_id))
                                            && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                                        let rows = composer_rows(&self.input_message);
                                        let text_edit =
                                            egui::TextEdit::multiline(&mut self.input_message)
                                                .font(FontId::proportional(14.0))
                                                .id(composer_id)
                                                .frame(false)
                                                .hint_text(if input_w<110.0 {"Nhập tin…".into()}else if input_w<180.0 {"Nhập tin nhắn…".into()}else{format!(
                                                    "Nhập tin nhắn tới {}...",
                                                    truncate_chars(&thread_name, 16)
                                                )})
                                                .margin(Margin::symmetric(6.0, 7.0))
                                                .desired_rows(rows)
                                                .desired_width(input_w);

                                        let response = egui::ScrollArea::vertical()
                                            .id_salt(("chat_composer_scroll", &thread_id))
                                            .max_width(input_w)
                                            .min_scrolled_width(input_w)
                                            .auto_shrink([false, true])
                                            .min_scrolled_height(0.0)
                                            .max_height(editor_height)
                                            .stick_to_bottom(true)
                                            .show(ui, |ui| ui.add(text_edit))
                                            .inner;
                                        if self.composer_focus_pending {
                                            response.request_focus();
                                            self.composer_focus_pending = false;
                                        }
                                        response.context_menu(|ui| {
                                            if ui
                                                .button(format!(
                                                    "{} Dán hình ảnh từ Clipboard (Ctrl+V)",
                                                    lucide::IMAGE
                                                ))
                                                .clicked()
                                            {
                                                ui.close_menu();
                                                self.request_clipboard_image();
                                            }
                                        });

                                        // Keyboard navigation for contextual sticker popover (Zalo PC `changeSelectedIndex`):
                                        // - ArrowUp selects the first suggested sticker
                                        // - ArrowLeft / ArrowRight cycles through suggested stickers
                                        // - ArrowDown / Escape deselects or hides the popover
                                        // - Enter sends the selected sticker if one is highlighted, otherwise sends text!
                                        let mut send_selected_sticker: Option<
                                            crate::chat::StickerItem,
                                        > = None;
                                        if !contextual_stickers.is_empty() && response.has_focus() {
                                            let len = contextual_stickers.len();
                                            if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                                                self.composer_sticker_selected = Some(0);
                                            } else if let Some(sel) = self.composer_sticker_selected
                                            {
                                                if ui.input(|i| i.key_pressed(Key::ArrowRight)) {
                                                    self.composer_sticker_selected =
                                                        Some((sel + 1) % len);
                                                } else if ui
                                                    .input(|i| i.key_pressed(Key::ArrowLeft))
                                                {
                                                    self.composer_sticker_selected =
                                                        Some((sel + len - 1) % len);
                                                } else if ui
                                                    .input(|i| i.key_pressed(Key::ArrowDown))
                                                {
                                                    self.composer_sticker_selected = None;
                                                }
                                            }
                                        } else {
                                            self.composer_sticker_selected = None;
                                        }

                                        let enter_pressed = enter_send;
                                        if enter_pressed {
                                            if let Some(sel) = self.composer_sticker_selected {
                                                if let Some(st) = contextual_stickers.get(sel) {
                                                    send_selected_sticker = Some(st.clone());
                                                }
                                            }
                                        }
                                        if !composer_compact {
                                            let (rect,button)=ui.allocate_exact_size(Vec2::splat(34.0),Sense::click());
                                            let c=rect.center();let stroke=Stroke::new(1.6_f32,pal.text_secondary);
                                            ui.painter().circle_stroke(c,8.0,stroke);
                                            ui.painter().line_segment([c+Vec2::new(0.0,-5.0),c],stroke);
                                            ui.painter().line_segment([c,c+Vec2::new(4.0,2.0)],stroke);
                                            if button.on_hover_text("Hẹn giờ gửi tin nhắn").on_hover_cursor(CursorIcon::PointingHand).clicked(){self.open_quick_schedule();}
                                        }
                                        // Responsive Padded Send Button (shows icon-only on very tight widths)
                                        let can_send = !self.input_message.trim().is_empty()
                                            || self.pending_clipboard_image.is_some();
                                        let (send_rect, send_resp) = ui.allocate_exact_size(
                                            Vec2::new(send_btn_w, 34.0),
                                            Sense::click(),
                                        );
                                        if self.smoke.is_some() && !ui.clip_rect().expand(1.0).contains_rect(send_rect) {
                                            eprintln!("UI smoke failed: send button outside composer viewport");std::process::exit(3);
                                        }
                                        let send_bg = if !can_send {
                                            pal.primary.linear_multiply(0.42)
                                        } else if send_resp.hovered() {
                                            pal.primary_hover
                                        } else {
                                            pal.primary
                                        };
                                        ui.painter().rect_filled(
                                            send_rect,
                                            Rounding::same(8.0),
                                            send_bg,
                                        );
                                        if !composer_compact {
                                            ui.painter().text(
                                                Pos2::new(
                                                    send_rect.left() + 15.0,
                                                    send_rect.center().y,
                                                ),
                                                egui::Align2::LEFT_CENTER,
                                                "Gửi",
                                                FontId::proportional(13.0),
                                                Color32::WHITE,
                                            );
                                            draw_icon_send(
                                                ui.painter(),
                                                Pos2::new(
                                                    send_rect.right() - 17.0,
                                                    send_rect.center().y,
                                                ),
                                                14.0,
                                                Color32::WHITE,
                                            );
                                        } else {
                                            draw_icon_send(
                                                ui.painter(),
                                                send_rect.center(),
                                                16.0,
                                                Color32::WHITE,
                                            );
                                        }


                                        send_resp.context_menu(|ui|{
                                            if ui.button("Nhãn dán…").clicked(){self.sticker_picker_open = !self.sticker_picker_open;ui.close_menu();}
                                            for (images_only,label) in [(true,"Gửi ảnh…"),(false,"Gửi tệp…")] {
                                                if ui.button(label).clicked(){
                                                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::PickAndSendAttachment{to_id:thread_id.clone(),is_group,images_only,caption:self.input_message.trim().to_owned(),preset_path:None});}
                                                    ui.close_menu();
                                                }
                                            }
                                            if ui.button("Hẹn giờ gửi…").clicked(){self.open_quick_schedule();ui.close_menu();}
                                            if ui.button("Quản lý lịch gửi").clicked(){self.tools.open=true;self.tools.tab=3;ui.close_menu();}
                                        });
                                        let send_clicked = send_resp
                                            .on_hover_cursor(if can_send {
                                                CursorIcon::PointingHand
                                            } else {
                                                CursorIcon::Default
                                            })
                                            .clicked();

                                        if let Some(chosen) = send_selected_sticker {
                                            self.input_message.clear();
                                            self.dismissed_sticker_suggest_for.clear();
                                            self.composer_sticker_selected = None;
                                            response.request_focus();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::SendSticker {
                                                    to_id: thread_id.clone(),
                                                    sticker_id: chosen.sticker_id,
                                                    cate_id: chosen.cate_id,
                                                    sticker_type: chosen.sticker_type,
                                                    is_group,
                                                });
                                            }
                                        } else if (send_clicked || enter_pressed)
                                            && self.pending_clipboard_image.is_some()
                                        {
                                            if let Some((clip_path, _, _, _)) =
                                                self.pending_clipboard_image.take()
                                            {
                                                let cap =
                                                    self.input_message.trim().to_string();
                                                self.input_message.clear();
                                                self.dismissed_sticker_suggest_for.clear();
                                                self.composer_sticker_selected = None;
                                                response.request_focus();
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ = tx.send(
                                                        WorkerCmd::PickAndSendAttachment {
                                                            to_id: thread_id.clone(),
                                                            is_group,
                                                            images_only: true,
                                                            caption: cap,
                                                            preset_path: Some(clip_path),
                                                        },
                                                    );
                                                }
                                            }
                                        } else if (send_clicked || enter_pressed) && can_send {
                                            let text = self.input_message.trim().to_string();
                                            self.input_message.clear();
                                            self.dismissed_sticker_suggest_for.clear();
                                            self.composer_sticker_selected = None;
                                            response.request_focus();
                                            let quoted = self.replying_to.take();
                                            if let Some(tx) = WORKER_TX.get() {
                                                if let Some(quoted) = quoted {
                                                    let _ = tx.send(WorkerCmd::SendQuote {
                                                        to_id: thread_id.clone(),
                                                        text,
                                                        quoted,
                                                        is_group,
                                                    });
                                                } else {
                                                    let _ = tx.send(WorkerCmd::SendMessage {
                                                        to_id: thread_id.clone(),
                                                        text,
                                                        is_group,
                                                    });
                                                }
                                            }
                                        } else if enter_pressed {
                                            response.request_focus();
                                        }
                                    });
                                });
                        });

                    // 4b. Floating Contextual Sticker Suggestion Popover (`egui::Area`, rendered AFTER composer bar)
                    if !contextual_stickers.is_empty() && !self.sticker_picker_open {
                        self.render_contextual_sticker_popover(
                            ctx,
                            composer_frame.response.rect,
                            &contextual_stickers,
                            &thread_id,
                            is_group,
                            pal,
                        );
                    }
                    if let Some(ref link_meta) = composer_link_preview {
                        if contextual_stickers.is_empty() && !self.sticker_picker_open {
                            self.render_composer_link_preview_popover(
                                ctx,
                                composer_frame.response.rect,
                                link_meta,
                                pal,
                            );
                        }
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(120.0);
                        ui.label(
                            RichText::new("Chọn một hội thoại bên trái để bắt đầu trò chuyện")
                                .size(15.0)
                                .color(pal.text_muted),
                        );
                    });
                }
            });

        self.render_workspace_overlays(ctx, pal);
    }

    /// Select a conversation, mark visible messages read and load server history when needed.
    pub(super) fn open_thread(&mut self, ctx: &egui::Context, idx: usize, tid: String, is_group: bool) {
        if self.selected_thread_index != idx {
            self.replying_to = None;
            self.filter_starred_only = false;
        }
        self.selected_thread_index = idx;
        self.sync_composer_draft(ctx);
        if let Some(thread) = self.threads.get_mut(idx) {
            thread.unread.read_visible(&thread.messages);
        }
        self.persist_threads_async();
        if self.unread_counts.remove(&tid).is_some() {
            let total_unread: usize = self.unread_counts.values().sum();
            let title = if total_unread > 0 {
                format!("({total_unread}) Zalo Lite — Native Rust Client")
            } else {
                "Zalo Lite — Native Rust Client".to_string()
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        let is_cloud = self
            .threads
            .get(idx)
            .map(|t| t.name.contains("Cloud của tôi"))
            .unwrap_or(false);
        if is_group || is_cloud {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::LoadGroupHistory {
                    limit: 500,
                    group_id: tid,
                    is_group,
                });
            }
        }
    }

    fn render_workspace_overlays(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        self.render_profile_popover(ctx, pal);
        self.render_toast_overlay(ctx, pal);
        self.render_forward_dialog(ctx, pal);
        self.render_call_overlay(ctx, pal);
        self.render_image_viewer_modal(ctx, pal);
        self.render_phone_lookup_modal(ctx, pal);
        self.render_settings_modal(ctx, pal);
        self.render_tools_panel(ctx);
        self.render_downloads(ctx);
        self.render_quick_schedule(ctx,pal);
    }
}

#[cfg(test)]
mod sync_and_order_tests {
    use super::*;
    fn app_and_context() -> (ZaloGuiApp, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.is_logged_in = true;
        (app, ctx)
    }
    fn thread(id: &str, group: bool, metadata: i64, times: &[i64]) -> Thread {
        Thread {
            id: id.into(),
            name: if id == "cloud" {
                "Cloud của tôi".into()
            } else {
                id.into()
            },
            is_group: group,
            messages: times
                .iter()
                .enumerate()
                .map(|(i, &ts)| ChatMessage {
                    id: format!("fixture-{i}"),
                    ts,
                    ..Default::default()
                })
                .collect(),
            presentation: crate::chat::SidebarPresentation {
                last_message_ts: metadata,
                ..Default::default()
            },
            preview: String::new(),
            time: String::new(),
            avatar: None,
            unread: Default::default(),
        }
    }
    #[test]
    fn phone_progress_is_visible_and_resets_with_account() {
        let (tx, rx) = std::sync::mpsc::channel();
        let (mut app, ctx) = app_and_context();
        app.event_rx = rx;
        tx.send(GuiEvent::PhoneSyncStatus {
            running: true,
            message: "Chờ xác nhận trên điện thoại…".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.phone_sync_running);
        assert_eq!(app.sync_status_message(), "Chờ xác nhận trên điện thoại…");
        app.reset_sync_status();
        assert!(!app.phone_sync_running);
        assert!(app.phone_sync_status.is_empty());
    }
    #[test]
    fn latest_group_or_direct_sorts_first_even_with_evicted_or_out_of_order_history() {
        let (mut app, _) = app_and_context();
        app.threads = vec![
            thread("cloud", false, 1_700_000_000, &[]),
            thread("group", true, 1_700_000_300_000, &[1_700_000_100]),
            thread("direct", false, 0, &[1_700_000_200_000, 1_700_000_050_000]),
        ];
        app.selected_thread_index = 2;
        let mut order = vec![0, 1, 2];
        app.sort_conversation_indices(&mut order);
        assert_eq!(order, vec![1, 2, 0]);
        assert_eq!(
            app.selected_thread_index, 2,
            "sorting the sidebar must not switch the open chat"
        );
        // Appending an older history page cannot demote the latest conversation.
        app.threads[1].messages.push(ChatMessage {
            id: "old-history".into(),
            ts: 1_600_000_000,
            ..Default::default()
        });
        app.sort_conversation_indices(&mut order);
        assert_eq!(order, vec![1, 2, 0]);
        app.threads[2].messages.push(ChatMessage {
            id: "new-live".into(),
            ts: 1_700_000_400,
            ..Default::default()
        });
        app.sort_conversation_indices(&mut order);
        assert_eq!(order, vec![2, 1, 0]);
    }
    #[test]
    fn explicit_pins_keep_priority_without_implicitly_pinning_cloud() {
        let (mut app, _) = app_and_context();
        app.threads = vec![
            thread("cloud", false, 1_700_000_000, &[]),
            thread("latest", true, 1_700_000_300, &[]),
        ];
        let mut order = vec![0, 1];
        app.sort_conversation_indices(&mut order);
        assert_eq!(order, vec![1, 0]);
        app.conversation_preferences.insert(
            "cloud".into(),
            crate::store::ConversationPreference {
                pinned: true,
                ..Default::default()
            },
        );
        app.sort_conversation_indices(&mut order);
        assert_eq!(order, vec![0, 1]);
    }
    #[test]
    fn main_status_tracks_progress_cancel_error_and_account_reset_without_settings() {
        let (mut app, ctx) = app_and_context();
        let (tx, rx) = channel();
        app.event_rx = rx;
        tx.send(GuiEvent::ConversationRefreshStatus {
            running: true,
            message: "Đang cập nhật hội thoại".into(),
        })
        .unwrap();
        tx.send(GuiEvent::HistorySyncStatus {
            running: true,
            message: "Nhóm thử — trang 2, 50 tin mới".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.sync_status_message(), "Nhóm thử — trang 2, 50 tin mới");
        assert!(!app.settings_modal_open);
        app.sync_cancel_requested = true;
        tx.send(GuiEvent::HistorySyncStatus {
            running: true,
            message: "Nhóm thử — trang 3, 100 tin mới".into(),
        })
        .unwrap();
        tx.send(GuiEvent::ConversationRefreshStatus {
            running: false,
            message: "Đã cập nhật hội thoại".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.sync_cancel_requested);
        assert_eq!(app.sync_status_message(), "Nhóm thử — trang 3, 100 tin mới");
        tx.send(GuiEvent::HistorySyncStatus {
            running: false,
            message: "Lỗi tải lịch sử: mất kết nối".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(!app.history_sync_running);
        assert!(!app.sync_cancel_requested);
        assert_eq!(app.sync_status_message(), "Lỗi tải lịch sử: mất kết nối");
        app.reset_sync_status();
        assert!(app.sync_status_message().is_empty());
        assert!(!app.refresh_sync_running);
    }
    #[test]
    fn main_status_bar_fits_small_and_scaled_workspaces() {
        let (mut app, ctx) = app_and_context();
        app.history_sync_running = true;
        app.history_sync_status = "Lịch sử 2/4: Nhóm thử nghiệm — trang 3, 125 tin mới".into();
        for scale in [1.0, 1.5, 2.0] {
            for size in [
                Vec2::new(320.0, 240.0),
                Vec2::new(480.0, 360.0),
                Vec2::new(1020.0, 700.0),
            ] {
                for _ in 0..2 {
                    let mut input = egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    };
                    input
                        .viewports
                        .get_mut(&egui::ViewportId::ROOT)
                        .unwrap()
                        .native_pixels_per_point = Some(scale);
                    let _ = ctx.run(input, |ctx| {
                        app.render_sync_status_bar(ctx, &app.palette());
                        assert!(
                            ctx.available_rect().height() > 0.0,
                            "sync notice must leave room for chat"
                        );
                        assert!(
                            ctx.available_rect().width() <= size.x + 1.0,
                            "scale={scale} size={size:?} screen={:?} available={:?}",
                            ctx.screen_rect(),
                            ctx.available_rect()
                        );
                    });
                }
            }
        }
    }
}

/// Maximum explicit rows before the composer scrolls.
const COMPOSER_MAX_ROWS: usize = 5;
/// Approximate laid-out height of one 14px composer row.
const COMPOSER_ROW_HEIGHT: f32 = 20.0;
/// Height of five rows plus the editor margin.
const COMPOSER_MAX_HEIGHT: f32 = 114.0;

fn composer_rows(text: &str) -> usize {
    text.split('\n').count().clamp(1, COMPOSER_MAX_ROWS)
}

/// Reserve visual wrapping too; explicit newline counts miss narrow editors.
fn composer_editor_height(ui:&egui::Ui,text:&str,width:f32)->f32 {
    let measured=ui.fonts(|f|f.layout(if text.is_empty(){" ".into()}else{text.to_owned()},FontId::proportional(14.0),Color32::WHITE,(width-12.0).max(1.0)).size().y);
    (measured.max(COMPOSER_ROW_HEIGHT)+14.0).min(COMPOSER_MAX_HEIGHT)
}

fn render_quote_target(ui: &egui::Ui, rect: egui::Rect, scroll_pending: bool, color: Color32) {
    ui.painter().rect_stroke(
        rect.expand(2.0),
        Rounding::same(8.0),
        Stroke::new(2.0_f32, color),
    );
    if scroll_pending {
        ui.scroll_to_rect(rect, Some(Align::Center));
    }
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(100));
}

#[cfg(test)]
mod quote_navigation_tests {
    use super::*;

    #[test]
    fn quote_navigation_invalidates_when_thread_changes_or_highlight_expires() {
        let messages = [ChatMessage {
            id: "original".into(),
            ..Default::default()
        }];
        let mut jump = QuoteJump {
            thread_id: "thread-1".into(),
            reference: "original".into(),
            highlight_until: std::time::Instant::now() + std::time::Duration::from_secs(3),
            scroll_pending: true,
        };
        assert_eq!(jump.target_index("thread-1", &messages), Some(0));
        assert_eq!(jump.target_index("thread-2", &messages), None);
        jump.highlight_until = std::time::Instant::now();
        assert_eq!(jump.target_index("thread-1", &messages), None);
    }

    #[test]
    fn quote_navigation_keeps_animated_jump_visible_after_highlight_expires() {
        for (count, target) in [(1000, 12), (1000, 900), (150, 12)] {
            let ctx = egui::Context::default();
            let color = Color32::from_rgb(1, 2, 3);
            let mut before = 0.0;
            let mut after = 0.0;
            let mut highlighted = false;
            let mut target_visible = false;
            for frame in 0..10 {
                let jumping = frame > 0;
                let highlighting = jumping && frame < 7;
                let limit = if jumping {
                    crate::message_navigation::render_limit_for_target(150, count, target)
                } else {
                    150
                };
                if count == 1000 && target == 12 {
                    assert!(jumping || count - limit > target);
                }
                let output = ctx.run(
                    egui::RawInput {
                        time: Some(frame as f64 * 0.5),
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(400.0, 300.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            let scroll = egui::ScrollArea::vertical()
                                .id_salt("quote_jump_test")
                                .max_height(200.0)
                                .stick_to_bottom(!highlighting)
                                .show(ui, |ui| {
                                    for index in count - limit..count {
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(200.0, 30.0),
                                            Sense::hover(),
                                        );
                                        if jumping && index == target {
                                            if highlighting {
                                                render_quote_target(ui, rect, frame == 1, color);
                                                highlighted = true;
                                            }
                                            if frame == 9 {
                                                target_visible =
                                                    ui.clip_rect().contains(rect.center());
                                            }
                                        }
                                    }
                                });
                            if jumping {
                                after = scroll.state.offset.y;
                            } else {
                                before = scroll.state.offset.y;
                            }
                        });
                    },
                );
                if frame == 4 {
                    assert!(output.shapes.iter().any(|shape| {
                    matches!(&shape.shape, egui::Shape::Rect(rect) if rect.stroke.color == color)
                }), "Exact target must have a visible highlight shape");
                }
            }
            assert!(highlighted);
            assert!(
                before > 1000.0,
                "Initial viewport must start at the newest messages"
            );
            assert!(
                after < before && target_visible,
                "Jump must center the older target, not remain at the bottom: {after}"
            );
        }
    }
}

#[cfg(test)]
mod composer_tests {
    use super::*;

    #[test]
    fn composer_rows_grow_with_lines_and_cap_at_five() {
        assert_eq!(composer_rows(""), 1);
        assert_eq!(composer_rows("một dòng"), 1);
        assert_eq!(composer_rows("a\nb\nc"), 3);
        assert_eq!(composer_rows("1\n2\n3\n4\n5\n6\n7"), 5);

    }
    #[test]
    fn wrapped_single_line_yields_message_space_and_stops_at_five_rows() {
        let ctx=egui::Context::default();
        let _=ctx.run(Default::default(),|ctx| {
            egui::CentralPanel::default().show(ctx,|ui| {
                let short=composer_editor_height(ui,"Hi",200.0);
                let wrapped=composer_editor_height(ui,&"Tiếng Việt có dấu và khoảng trắng. ".repeat(12),80.0);
                assert!(wrapped>short);
                assert_eq!(wrapped,COMPOSER_MAX_HEIGHT);
            });
        });
    }

}
