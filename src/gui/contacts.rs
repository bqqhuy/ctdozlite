//! Contacts page: friends, groups, friend requests, and group invitations.
//!
//! Sub-navigation sidebar (280px) with 4 standard Zalo PC tabs:
//! 1. Friends (A-Z grouped with alphabet jump bar)
//! 2. Groups & Communities
//! 3. Friend Requests (Incoming & Outgoing management)
//! 4. Group Invitations & Join by Link

use super::*;

/// Contacts page navigation tab.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ContactsTab {
    #[default]
    Friends,
    Groups,
    FriendRequests,
    GroupInvitations,
}

/// Group thread indices into alphabetical sections by diacritic-folded display name,
/// prioritizing saved `aliases` for 1-1 contacts.
///
/// Excludes Cloud, hidden conversations and threads of the other kind. Letters outside A–Z
/// share the `#` section, which sorts last.
pub(super) fn contact_sections_with_aliases(
    threads: &[Thread],
    preferences: &HashMap<String, crate::store::ConversationPreference>,
    aliases: &HashMap<String, String>,
    tab: ContactsTab,
    query: &str,
) -> Vec<(char, Vec<usize>)> {
    let query = fold_vn_search(query.trim());
    let is_group_target = tab == ContactsTab::Groups;
    let mut entries: Vec<(String, usize)> = threads
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_group == is_group_target)
        .filter(|(_, t)| !t.id.is_empty() && !t.name.contains("Cloud của tôi"))
        .filter(|(_, t)| !preferences.get(&t.id).is_some_and(|p| p.hidden))
        .filter_map(|(i, t)| {
            let resolved = if t.is_group {
                t.name.as_str()
            } else {
                resolve_contact_name_from(aliases, &t.id, &t.name)
            };
            let folded = fold_vn_search(resolved.trim());
            let base_matches = t
                .presentation
                .base_name
                .as_deref()
                .is_some_and(|b| fold_vn_search(b.trim()).contains(&query));
            (query.is_empty() || folded.contains(&query) || base_matches).then_some((folded, i))
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut sections: Vec<(char, Vec<usize>)> = Vec::new();
    for (name, index) in entries {
        let letter = name
            .chars()
            .next()
            .filter(char::is_ascii_alphabetic)
            .map_or('#', |c| c.to_ascii_uppercase());
        match sections.iter_mut().find(|(l, _)| *l == letter) {
            Some((_, items)) => items.push(index),
            None => sections.push((letter, vec![index])),
        }
    }
    sections.sort_by_key(|(letter, _)| (*letter == '#', *letter));
    sections
}

/// Group thread indices into alphabetical sections by diacritic-folded name.
///
/// Excludes Cloud, hidden conversations and threads of the other kind. Letters outside A–Z
/// share the `#` section, which sorts last.
#[cfg(test)]
pub(super) fn contact_sections(
    threads: &[Thread],
    preferences: &HashMap<String, crate::store::ConversationPreference>,
    tab: ContactsTab,
    query: &str,
) -> Vec<(char, Vec<usize>)> {
    static EMPTY: std::sync::OnceLock<HashMap<String, String>> = std::sync::OnceLock::new();
    contact_sections_with_aliases(
        threads,
        preferences,
        EMPTY.get_or_init(HashMap::new),
        tab,
        query,
    )
}

impl ZaloGuiApp {
    fn visible_contact_sections(&self, tab: ContactsTab, query: &str) -> Vec<(char, Vec<usize>)> {
        let mut sections = contact_sections_with_aliases(
            &self.threads,
            &self.conversation_preferences,
            &self.server_aliases,
            tab,
            query,
        );
        for (_, indices) in &mut sections {
            indices.retain(|index| {
                let thread = &self.threads[*index];
                self.private_access(&thread.id, thread.is_group)
                    && (thread.is_group
                        || self.settings.show_all_contacts
                        || self
                            .inbox_policy
                            .friends
                            .as_ref()
                            .is_none_or(|friends| friends.contains(&thread.id)))
            });
        }
        sections.retain(|(_, indices)| !indices.is_empty());
        sections
    }

    pub(super) fn render_contacts_page(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let count = |tab| {
            self.visible_contact_sections(tab, "")
                .iter()
                .map(|(_, items)| items.len())
                .sum::<usize>()
        };
        let friends = count(ContactsTab::Friends);
        let groups = count(ContactsTab::Groups);
        let friend_requests_count = self
            .friend_status_map
            .iter()
            .filter(|(uid, status)| {
                **status == crate::chat::FriendshipStatus::IncomingPending
                    && self.private_access(uid, false)
            })
            .count();
        let group_invitations_count = self
            .group_entry_panel
            .page
            .as_ref()
            .map(|p| p.invitations.len())
            .unwrap_or(0);

        // 1. LEFT CONTACTS SUB-NAVIGATION SIDEBAR (280px)
        egui::SidePanel::left("contacts_sidebar")
            .resizable(false)
            .exact_width(280.0)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::symmetric(14.0, 16.0)),
            )
            .show(ctx, |ui| {
                // Header with title and [+👤 Thêm bạn], [👥+ Tạo nhóm]
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(crate::i18n::tr("Danh bạ"))
                            .size(17.5)
                            .strong()
                            .color(pal.text_primary),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        // Create Group button [👥+]
                        let grp_btn = egui::Button::new(
                            RichText::new(format!("{} +", lucide::USERS))
                                .size(12.5)
                                .color(pal.text_primary),
                        )
                        .fill(pal.bg_input)
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .rounding(Rounding::same(6.0))
                        .min_size(Vec2::new(32.0, 28.0));
                        if ui
                            .add(grp_btn)
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(crate::i18n::tr("Tạo nhóm mới"))
                            .clicked()
                        {
                            self.open_group_panel(String::new(), true, true);
                        }

                        // Add Friend / Phone Lookup button [+👤]
                        let add_btn = egui::Button::new(
                            RichText::new(format!("+ {}", lucide::USER))
                                .size(12.5)
                                .color(pal.text_primary),
                        )
                        .fill(pal.bg_input)
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .rounding(Rounding::same(6.0))
                        .min_size(Vec2::new(32.0, 28.0));
                        let add_resp = ui
                            .add(add_btn)
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(crate::i18n::tr(
                                "Thêm bạn bè / Tìm số điện thoại (Chuột phải: Tra cứu username)",
                            ));
                        if add_resp.clicked() {
                            self.phone_modal_open = true;
                        } else if add_resp.secondary_clicked() {
                            self.open_username_lookup();
                        }
                    });
                });

                ui.add_space(10.0);

                // Search field
                ui.horizontal(|ui| {
                    let te = egui::TextEdit::singleline(&mut self.contacts_query)
                        .id(Id::new("contacts_search_input"))
                        .hint_text(crate::i18n::tr("Tìm bạn bè, nhóm..."))
                        .margin(Margin::symmetric(10.0, 6.0))
                        .desired_width(ui.available_width());
                    ui.add(te);
                });

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);

                // 4 Navigation Items
                // Use the shared icon font for navigation affordances. Emoji glyphs
                // depend on the host font stack and rendered as tofu on some ARM64
                // Windows installations.
                let nav_items = [
                    (
                        ContactsTab::Friends,
                        lucide::USERS,
                        "Danh sách bạn bè",
                        friends,
                    ),
                    (ContactsTab::Groups, lucide::USERS, "Danh sách nhóm", groups),
                    (
                        ContactsTab::FriendRequests,
                        lucide::USER_PLUS,
                        "Lời mời kết bạn",
                        friend_requests_count,
                    ),
                    (
                        ContactsTab::GroupInvitations,
                        lucide::MESSAGE_SQUARE,
                        "Lời mời vào nhóm",
                        group_invitations_count,
                    ),
                ];

                for (tab, icon, label, total) in nav_items {
                    let active = self.contacts_tab == tab;
                    let (rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click());
                    if active {
                        ui.painter().rect_filled(
                            rect,
                            Rounding::same(8.0),
                            pal.primary.linear_multiply(0.18),
                        );
                        ui.painter().rect_filled(
                            Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height())),
                            Rounding {
                                nw: 2.0,
                                sw: 2.0,
                                ne: 0.0,
                                se: 0.0,
                            },
                            pal.primary,
                        );
                    } else if response.hovered() {
                        ui.painter()
                            .rect_filled(rect, Rounding::same(8.0), pal.bg_hover);
                    }

                    // Icon
                    ui.painter().text(
                        Pos2::new(rect.left() + 14.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        icon,
                        FontId::proportional(15.0),
                        if active {
                            pal.primary_hover
                        } else {
                            pal.text_secondary
                        },
                    );

                    // Label
                    ui.painter().text(
                        Pos2::new(rect.left() + 38.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        FontId::proportional(13.2),
                        if active {
                            pal.primary_hover
                        } else {
                            pal.text_primary
                        },
                    );

                    // Count Badge
                    if total > 0 || matches!(tab, ContactsTab::Friends | ContactsTab::Groups) {
                        let badge_text = if total > 99 {
                            "99+".to_string()
                        } else {
                            total.to_string()
                        };
                        let badge_color = if active {
                            pal.primary_hover
                        } else {
                            pal.text_muted
                        };
                        ui.painter().text(
                            Pos2::new(rect.right() - 14.0, rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            badge_text,
                            FontId::proportional(11.8),
                            badge_color,
                        );
                    }

                    if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.contacts_tab = tab;
                        if tab == ContactsTab::GroupInvitations
                            && self.group_entry_panel.page.is_none()
                            && !self.group_entry_panel.pending
                        {
                            self.open_group_entry();
                        }
                    }
                    ui.add_space(4.0);
                }
            });

        // 2. CENTRAL CONTENT CANVAS
        let mut open_thread_idx: Option<usize> = None;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_chat_canvas)
                    .inner_margin(Margin::symmetric(24.0, 18.0)),
            )
            .show(ctx, |ui| match self.contacts_tab {
                ContactsTab::Friends => {
                    self.render_contacts_friends_tab(ctx, ui, pal, &mut open_thread_idx);
                }
                ContactsTab::Groups => {
                    self.render_contacts_groups_tab(ctx, ui, pal, &mut open_thread_idx);
                }
                ContactsTab::FriendRequests => {
                    self.render_contacts_friend_requests_tab(ctx, ui, pal, &mut open_thread_idx);
                }
                ContactsTab::GroupInvitations => {
                    self.render_contacts_group_invitations_tab(ctx, ui, pal);
                }
            });

        if let Some(index) = open_thread_idx {
            if let Some(thread) = self.threads.get(index) {
                let (id, is_group) = (thread.id.clone(), thread.is_group);
                self.contacts_view_open = false;
                self.open_thread(ctx, index, id, is_group);
            }
        }
    }

    // -------------------------------------------------------------------------
    // TAB 1: FRIENDS (A-Z with Alphabet Jump Bar)
    // -------------------------------------------------------------------------
    fn render_contacts_friends_tab(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        pal: &ThemePalette,
        open_thread_idx: &mut Option<usize>,
    ) {
        let sections = self.visible_contact_sections(ContactsTab::Friends, &self.contacts_query);
        let total: usize = sections.iter().map(|(_, items)| items.len()).sum();

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} ({total})",
                    if self.settings.show_all_contacts {
                        "Danh bạ đã liên hệ"
                    } else {
                        "Danh sách bạn bè"
                    }
                ))
                .size(16.0)
                .strong()
                .color(pal.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("Sắp xếp theo tên A-Z")
                        .size(11.8)
                        .color(pal.text_muted),
                );
            });
        });
        ui.add_space(8.0);

        // Alphabet Quick Jump Bar
        let available_letters: Vec<char> = sections.iter().map(|(l, _)| *l).collect();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
            for &ch in &[
                'A', 'B', 'C', 'D', 'E', 'G', 'H', 'I', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
                'S', 'T', 'U', 'V', 'X', 'Y', '#',
            ] {
                let has = available_letters.contains(&ch);
                let btn = egui::Button::new(
                    RichText::new(ch.to_string())
                        .size(10.8)
                        .color(if has {
                            pal.primary_hover
                        } else {
                            pal.text_muted
                        })
                        .strong(),
                )
                .fill(if has {
                    pal.bg_elevated
                } else {
                    Color32::TRANSPARENT
                })
                .rounding(Rounding::same(4.0))
                .min_size(Vec2::new(20.0, 20.0));
                let resp = ui.add_enabled(has, btn);
                if resp.clicked() {
                    // Quick letter filter: user can type or search if needed
                }
            }
        });
        ui.add_space(10.0);

        if sections.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new(if self.contacts_query.trim().is_empty() {
                        "Chưa có liên hệ bạn bè nào được tải."
                    } else {
                        "Không tìm thấy bạn bè phù hợp."
                    })
                    .size(13.5)
                    .color(pal.text_muted),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("contacts_friends_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (letter, items) in &sections {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        let (dot_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                        ui.painter().rect_filled(
                            dot_rect,
                            Rounding::same(4.0),
                            pal.primary.linear_multiply(0.14),
                        );
                        ui.painter().text(
                            dot_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            letter.to_string(),
                            FontId::proportional(12.0),
                            pal.primary_hover,
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(format!("Nhóm chữ cái {letter}"))
                                .size(12.0)
                                .color(pal.text_secondary),
                        );
                    });
                    ui.add_space(4.0);

                    for &index in items {
                        let Some(thread) = self.threads.get(index) else {
                            continue;
                        };
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 52.0),
                            Sense::click(),
                        );
                        if response.hovered() {
                            ui.painter()
                                .rect_filled(rect, Rounding::same(8.0), pal.bg_hover);
                        }

                        let display_name = self.resolve_contact_name(&thread.id, &thread.name);

                        // Avatar
                        let center = Pos2::new(rect.left() + 28.0, rect.center().y);
                        ui.painter().circle_filled(
                            center,
                            18.0,
                            avatar_color_for(display_name, false),
                        );
                        ui.painter().text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            avatar_initials(display_name),
                            FontId::proportional(13.0),
                            Color32::WHITE,
                        );

                        // Name
                        ui.painter().text(
                            Pos2::new(rect.left() + 58.0, rect.center().y - 8.0),
                            egui::Align2::LEFT_CENTER,
                            display_name,
                            FontId::proportional(13.8),
                            pal.text_primary,
                        );

                        // Subtitle
                        ui.painter().text(
                            Pos2::new(rect.left() + 58.0, rect.center().y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            "Bạn bè",
                            FontId::proportional(11.2),
                            pal.text_muted,
                        );

                        // Chat button on right
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(rect.right() - 90.0, rect.center().y - 14.0),
                            Vec2::new(80.0, 28.0),
                        );
                        let btn_hovered = ui.rect_contains_pointer(btn_rect);
                        ui.painter().rect_filled(
                            btn_rect,
                            Rounding::same(6.0),
                            if btn_hovered {
                                pal.primary.linear_multiply(0.2)
                            } else {
                                pal.bg_elevated
                            },
                        );
                        ui.painter().rect_stroke(
                            btn_rect,
                            Rounding::same(6.0),
                            Stroke::new(1.0_f32, pal.border),
                        );
                        ui.painter().text(
                            btn_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "💬 Nhắn tin",
                            FontId::proportional(12.0),
                            pal.text_primary,
                        );

                        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            *open_thread_idx = Some(index);
                        }
                    }
                }
            });
    }

    // -------------------------------------------------------------------------
    // TAB 2: GROUPS & COMMUNITIES
    // -------------------------------------------------------------------------
    fn render_contacts_groups_tab(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        pal: &ThemePalette,
        open_thread_idx: &mut Option<usize>,
    ) {
        let sections = self.visible_contact_sections(ContactsTab::Groups, &self.contacts_query);
        let total: usize = sections.iter().map(|(_, items)| items.len()).sum();

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("Danh sách nhóm & cộng đồng ({total})"))
                    .size(16.0)
                    .strong()
                    .color(pal.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("Các nhóm bạn đang tham gia")
                        .size(11.8)
                        .color(pal.text_muted),
                );
            });
        });
        ui.add_space(10.0);

        if sections.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new(if self.contacts_query.trim().is_empty() {
                        "Bạn chưa tham gia nhóm nào."
                    } else {
                        "Không tìm thấy nhóm phù hợp."
                    })
                    .size(13.5)
                    .color(pal.text_muted),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("contacts_groups_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (letter, items) in &sections {
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!("Chữ cái {letter}"))
                            .size(12.0)
                            .strong()
                            .color(pal.text_secondary),
                    );
                    ui.add_space(4.0);

                    for &index in items {
                        let Some(thread) = self.threads.get(index) else {
                            continue;
                        };
                        let (rect, response) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 54.0),
                            Sense::click(),
                        );
                        if response.hovered() {
                            ui.painter()
                                .rect_filled(rect, Rounding::same(8.0), pal.bg_hover);
                        }

                        // Avatar
                        let center = Pos2::new(rect.left() + 28.0, rect.center().y);
                        ui.painter().circle_filled(
                            center,
                            18.0,
                            avatar_color_for(&thread.name, true),
                        );
                        ui.painter().text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            avatar_initials(&thread.name),
                            FontId::proportional(13.0),
                            Color32::WHITE,
                        );

                        // Group Name
                        ui.painter().text(
                            Pos2::new(rect.left() + 58.0, rect.center().y - 8.0),
                            egui::Align2::LEFT_CENTER,
                            &thread.name,
                            FontId::proportional(13.8),
                            pal.text_primary,
                        );

                        // Member count badge
                        let member_count = self
                            .group_members
                            .get(&thread.id)
                            .map(|(m, _)| format!("{} thành viên", m.len()))
                            .unwrap_or_else(|| "Nhóm".into());
                        ui.painter().text(
                            Pos2::new(rect.left() + 58.0, rect.center().y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            member_count,
                            FontId::proportional(11.2),
                            pal.text_muted,
                        );

                        // Chat button
                        let btn_rect = Rect::from_min_size(
                            Pos2::new(rect.right() - 90.0, rect.center().y - 14.0),
                            Vec2::new(80.0, 28.0),
                        );
                        let btn_hovered = ui.rect_contains_pointer(btn_rect);
                        ui.painter().rect_filled(
                            btn_rect,
                            Rounding::same(6.0),
                            if btn_hovered {
                                pal.primary.linear_multiply(0.2)
                            } else {
                                pal.bg_elevated
                            },
                        );
                        ui.painter().rect_stroke(
                            btn_rect,
                            Rounding::same(6.0),
                            Stroke::new(1.0_f32, pal.border),
                        );
                        ui.painter().text(
                            btn_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "💬 Nhắn tin",
                            FontId::proportional(12.0),
                            pal.text_primary,
                        );

                        if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                            *open_thread_idx = Some(index);
                        }
                    }
                }
            });
    }

    // -------------------------------------------------------------------------
    // TAB 3: FRIEND REQUESTS (Incoming & Outgoing)
    // -------------------------------------------------------------------------
    fn render_contacts_friend_requests_tab(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        pal: &ThemePalette,
        open_thread_idx: &mut Option<usize>,
    ) {
        ui.label(
            RichText::new("Lời mời kết bạn")
                .size(16.0)
                .strong()
                .color(pal.text_primary),
        );
        ui.label(
            RichText::new("Quản lý yêu cầu kết bạn nhận được và yêu cầu bạn đã gửi")
                .size(12.0)
                .color(pal.text_secondary),
        );
        ui.add_space(14.0);

        // Collect incoming requests
        let mut incoming_requests: Vec<(Option<usize>, String, String)> = self
            .threads
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_group)
            .filter(|(_, t)| self.private_access(&t.id, false))
            .filter(|(_, t)| {
                self.friend_status_map.get(&t.id)
                    == Some(&crate::chat::FriendshipStatus::IncomingPending)
            })
            .map(|(i, t)| {
                (
                    Some(i),
                    t.id.clone(),
                    self.resolve_contact_name(&t.id, &t.name).to_owned(),
                )
            })
            .collect();

        for (uid, &status) in &self.friend_status_map {
            if status == crate::chat::FriendshipStatus::IncomingPending
                && self.private_access(uid, false)
                && !incoming_requests.iter().any(|(_, tid, _)| tid == uid)
            {
                let name = self.resolve_contact_name(uid, uid).to_owned();
                incoming_requests.push((None, uid.clone(), name));
            }
        }

        // Collect outgoing requests
        let mut outgoing_requests: Vec<(Option<usize>, String, String)> = self
            .threads
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.is_group)
            .filter(|(_, t)| self.private_access(&t.id, false))
            .filter(|(_, t)| {
                self.friend_status_map.get(&t.id)
                    == Some(&crate::chat::FriendshipStatus::OutgoingPending)
            })
            .map(|(i, t)| {
                (
                    Some(i),
                    t.id.clone(),
                    self.resolve_contact_name(&t.id, &t.name).to_owned(),
                )
            })
            .collect();

        for (uid, &status) in &self.friend_status_map {
            if status == crate::chat::FriendshipStatus::OutgoingPending
                && self.private_access(uid, false)
                && !outgoing_requests.iter().any(|(_, tid, _)| tid == uid)
            {
                let name = self.resolve_contact_name(uid, uid).to_owned();
                outgoing_requests.push((None, uid.clone(), name));
            }
        }

        egui::ScrollArea::vertical()
            .id_salt("contacts_friend_requests_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Section 1: Incoming
                ui.label(
                    RichText::new(format!("Lời mời nhận được ({})", incoming_requests.len()))
                        .size(13.8)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.add_space(6.0);

                if incoming_requests.is_empty() {
                    egui::Frame::none()
                        .fill(pal.bg_elevated)
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .inner_margin(Margin::same(14.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Không có lời mời kết bạn nào đang chờ duyệt.")
                                    .size(12.5)
                                    .color(pal.text_muted),
                            );
                        });
                } else {
                    for (t_idx, uid, name) in &incoming_requests {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .rounding(Rounding::same(10.0))
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(14.0, 10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (dr, _) =
                                        ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                                    ui.painter().circle_filled(
                                        dr.center(),
                                        18.0,
                                        avatar_color_for(name, false),
                                    );
                                    ui.painter().text(
                                        dr.center(),
                                        egui::Align2::CENTER_CENTER,
                                        avatar_initials(name),
                                        FontId::proportional(12.5),
                                        Color32::WHITE,
                                    );
                                    ui.add_space(8.0);

                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(name)
                                                .size(13.5)
                                                .strong()
                                                .color(pal.text_primary),
                                        );
                                        ui.label(
                                            RichText::new("Đã gửi cho bạn một lời mời kết bạn")
                                                .size(11.5)
                                                .color(pal.primary_hover),
                                        );
                                    });

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;

                                        // Accept button
                                        let acc_btn = egui::Button::new(
                                            RichText::new("✓ Đồng ý")
                                                .size(12.0)
                                                .color(Color32::WHITE)
                                                .strong(),
                                        )
                                        .fill(pal.accent_online)
                                        .rounding(Rounding::same(6.0))
                                        .min_size(Vec2::new(76.0, 28.0));
                                        if ui
                                            .add_enabled(!self.friend_action_pending(uid), acc_btn)
                                            .clicked()
                                        {
                                            self.request_friend_action(uid, FriendAction::Accept);
                                        }

                                        // Reject button
                                        let rej_btn = egui::Button::new(
                                            RichText::new("Từ chối")
                                                .size(12.0)
                                                .color(pal.text_secondary),
                                        )
                                        .fill(pal.bg_input)
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .rounding(Rounding::same(6.0))
                                        .min_size(Vec2::new(68.0, 28.0));
                                        if ui
                                            .add_enabled(!self.friend_action_pending(uid), rej_btn)
                                            .clicked()
                                        {
                                            self.request_friend_action(uid, FriendAction::Reject);
                                        }

                                        // Optional chat button if thread exists
                                        if let Some(idx) = t_idx {
                                            if ui.button("💬 Trò chuyện").clicked() {
                                                *open_thread_idx = Some(*idx);
                                            }
                                        }
                                    });
                                });
                            });
                        ui.add_space(6.0);
                    }
                }

                ui.add_space(16.0);

                // Section 2: Outgoing
                ui.label(
                    RichText::new(format!("Lời mời đã gửi ({})", outgoing_requests.len()))
                        .size(13.8)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.add_space(6.0);

                if outgoing_requests.is_empty() {
                    egui::Frame::none()
                        .fill(pal.bg_elevated)
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .inner_margin(Margin::same(14.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Bạn chưa gửi lời mời kết bạn nào đang chờ.")
                                    .size(12.5)
                                    .color(pal.text_muted),
                            );
                        });
                } else {
                    for (_, uid, name) in &outgoing_requests {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .rounding(Rounding::same(10.0))
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(14.0, 10.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (dr, _) =
                                        ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                                    ui.painter().circle_filled(
                                        dr.center(),
                                        18.0,
                                        avatar_color_for(name, false),
                                    );
                                    ui.painter().text(
                                        dr.center(),
                                        egui::Align2::CENTER_CENTER,
                                        avatar_initials(name),
                                        FontId::proportional(12.5),
                                        Color32::WHITE,
                                    );
                                    ui.add_space(8.0);

                                    ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(name)
                                                .size(13.5)
                                                .strong()
                                                .color(pal.text_primary),
                                        );
                                        ui.label(
                                            RichText::new("Đang chờ đối phương chấp nhận lời mời")
                                                .size(11.5)
                                                .color(pal.text_muted),
                                        );
                                    });

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let undo_btn = egui::Button::new(
                                            RichText::new("Thu hồi").size(12.0).color(pal.warning),
                                        )
                                        .fill(pal.bg_input)
                                        .stroke(Stroke::new(1.0_f32, pal.border))
                                        .rounding(Rounding::same(6.0))
                                        .min_size(Vec2::new(76.0, 28.0));
                                        if ui
                                            .add_enabled(!self.friend_action_pending(uid), undo_btn)
                                            .clicked()
                                        {
                                            self.request_friend_action(uid, FriendAction::Undo);
                                        }
                                    });
                                });
                            });
                        ui.add_space(6.0);
                    }
                }

                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("🔍 Tìm kiếm bạn mới bằng số điện thoại").size(13.0))
                        .clicked()
                    {
                        self.phone_modal_open = true;
                    }
                    if ui
                        .button(RichText::new("👤 Tra cứu username / Bạn thân").size(13.0))
                        .clicked()
                    {
                        self.open_username_lookup();
                    }
                });
            });
    }

    // -------------------------------------------------------------------------
    // TAB 4: GROUP INVITATIONS & JOIN BY LINK (In-Page, Zero Floating Windows)
    // -------------------------------------------------------------------------
    fn render_contacts_group_invitations_tab(
        &mut self,
        _ctx: &egui::Context,
        ui: &mut egui::Ui,
        pal: &ThemePalette,
    ) {
        let mut intent = None;

        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Lời mời vào nhóm & Tham gia qua Link")
                    .size(16.0)
                    .strong()
                    .color(pal.text_primary),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.group_entry_panel.pending,
                        egui::Button::new("🔄 Tải lại lời mời"),
                    )
                    .clicked()
                {
                    intent = Some(crate::group_entry::Action::Invitations {
                        page: 0,
                        last: None,
                    });
                }
            });
        });
        ui.add_space(4.0);

        if !self.group_entry_panel.status.is_empty() {
            ui.label(
                RichText::new(&self.group_entry_panel.status)
                    .size(12.0)
                    .color(pal.warning),
            );
        }
        if self.group_entry_panel.pending {
            ui.spinner();
        }
        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .id_salt("contacts_group_invites_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Section 1: Invitations List
                ui.label(
                    RichText::new("Lời mời từ các nhóm")
                        .size(13.8)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.add_space(6.0);

                if let Some(page) = &self.group_entry_panel.page {
                    ui.label(
                        RichText::new(format!(
                            "Trang {} • {} lời mời trên máy chủ",
                            page.page + 1,
                            page.total
                        ))
                        .size(12.0)
                        .color(pal.text_secondary),
                    );
                    ui.add_space(6.0);

                    if page.invitations.is_empty() {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .rounding(Rounding::same(8.0))
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::same(14.0))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new("Bạn hiện không có lời mời tham gia nhóm nào.")
                                        .size(12.5)
                                        .color(pal.text_muted),
                                );
                            });
                    } else {
                        for invitation in &page.invitations {
                            egui::Frame::none()
                                .fill(pal.bg_elevated)
                                .rounding(Rounding::same(10.0))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .inner_margin(Margin::symmetric(14.0, 10.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let (dr, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                                        ui.painter().circle_filled(
                                            dr.center(),
                                            18.0,
                                            avatar_color_for(&invitation.name, true),
                                        );
                                        ui.painter().text(
                                            dr.center(),
                                            egui::Align2::CENTER_CENTER,
                                            avatar_initials(&invitation.name),
                                            FontId::proportional(12.5),
                                            Color32::WHITE,
                                        );
                                        ui.add_space(8.0);

                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(&invitation.name)
                                                    .size(13.5)
                                                    .strong()
                                                    .color(pal.text_primary),
                                            );
                                            let expired = invitation.expires <= crate::login::unix_now() * 1000;
                                            if invitation.pending {
                                                ui.label(
                                                    RichText::new("Đang chờ quản trị viên duyệt")
                                                        .size(11.5)
                                                        .color(pal.warning),
                                                );
                                            } else if expired {
                                                ui.label(
                                                    RichText::new("Lời mời đã hết hạn")
                                                        .size(11.5)
                                                        .color(pal.text_muted),
                                                );
                                            } else {
                                                ui.label(
                                                    RichText::new(format!("Mã nhóm: {}", invitation.group))
                                                        .size(11.5)
                                                        .color(pal.text_muted),
                                                );
                                            }
                                        });

                                        let expired = invitation.expires <= crate::login::unix_now() * 1000;
                                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                            ui.spacing_mut().item_spacing.x = 6.0;
                                            ui.add_enabled_ui(!expired && !invitation.pending && !self.group_entry_panel.uncertain, |ui| {
                                                if ui.button(RichText::new("Tham gia").color(Color32::WHITE).strong())
                                                    .clicked()
                                                {
                                                    self.group_entry_panel.confirm = Some(crate::group_entry::Action::JoinInvitation {
                                                        group: invitation.group.clone(),
                                                    });
                                                }
                                                if ui.button("Từ chối").clicked() {
                                                    self.group_entry_panel.confirm = Some(crate::group_entry::Action::RejectInvitation {
                                                        group: invitation.group.clone(),
                                                        block_group: self.group_entry_panel.block_group,
                                                    });
                                                }
                                            });
                                        });
                                    });
                                });
                            ui.add_space(6.0);
                        }

                        ui.checkbox(
                            &mut self.group_entry_panel.block_group,
                            "Kèm chặn lời mời từ nhóm khi từ chối (không chặn người mời)",
                        );
                        if page.more && ui.button("Trang lời mời tiếp").clicked() {
                            intent = Some(crate::group_entry::Action::Invitations {
                                page: page.page + 1,
                                last: page.last.clone(),
                            });
                        }
                    }
                } else {
                    egui::Frame::none()
                        .fill(pal.bg_elevated)
                        .rounding(Rounding::same(8.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .inner_margin(Margin::same(14.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("Bấm “Tải lại lời mời” để đồng bộ lời mời nhóm từ máy chủ.")
                                    .size(12.5)
                                    .color(pal.text_muted),
                            );
                        });
                }

                ui.add_space(18.0);
                ui.separator();
                ui.add_space(14.0);

                // Section 2: Join Group by Link
                ui.label(
                    RichText::new("Tham gia nhóm bằng Link")
                        .size(13.8)
                        .strong()
                        .color(pal.text_primary),
                );
                ui.label(
                    RichText::new("Dán link mời tham gia nhóm Zalo để xem trước thông tin và gửi yêu cầu.")
                        .size(12.0)
                        .color(pal.text_secondary),
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut self.group_entry_panel.link)
                                .hint_text("Dán link nhóm tại đây (VD: https://zalo.me/g/...)")
                                .desired_width(ui.available_width() - 170.0),
                        )
                        .changed()
                    {
                        self.group_entry_panel.preview = None;
                        self.group_entry_panel.preview_link.clear();
                        self.group_entry_panel.confirm = None;
                    }

                    if ui
                        .add_enabled(
                            !self.group_entry_panel.link.trim().is_empty(),
                            egui::Button::new("Xem thông tin link"),
                        )
                        .clicked()
                    {
                        self.group_entry_panel.preview_link = self.group_entry_panel.link.clone();
                        intent = Some(crate::group_entry::Action::LinkInfo {
                            link: self.group_entry_panel.link.clone(),
                        });
                    }
                });

                if let Some(preview) = &self.group_entry_panel.preview {
                    ui.add_space(8.0);
                    egui::Frame::none()
                        .fill(pal.bg_elevated)
                        .rounding(Rounding::same(10.0))
                        .stroke(Stroke::new(1.0_f32, pal.primary))
                        .inner_margin(Margin::same(12.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (dr, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                                ui.painter().circle_filled(
                                    dr.center(),
                                    18.0,
                                    avatar_color_for(&preview.name, true),
                                );
                                ui.painter().text(
                                    dr.center(),
                                    egui::Align2::CENTER_CENTER,
                                    avatar_initials(&preview.name),
                                    FontId::proportional(12.5),
                                    Color32::WHITE,
                                );
                                ui.add_space(8.0);

                                ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&preview.name)
                                            .size(13.8)
                                            .strong()
                                            .color(pal.text_primary),
                                    );
                                    if let Some(count) = preview.members {
                                        ui.label(
                                            RichText::new(format!("{count} thành viên"))
                                                .size(11.5)
                                                .color(pal.text_muted),
                                        );
                                    }
                                });

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if !self.group_entry_panel.uncertain
                                        && self.group_entry_panel.link == self.group_entry_panel.preview_link
                                        && ui
                                            .button(
                                                RichText::new("Yêu cầu tham gia nhóm qua link")
                                                    .color(Color32::WHITE)
                                                    .strong(),
                                            )
                                            .clicked()
                                    {
                                        self.group_entry_panel.confirm = Some(crate::group_entry::Action::JoinLink {
                                            link: self.group_entry_panel.link.clone(),
                                        });
                                    }
                                });
                            });
                        });
                }

                // Confirmation banner
                if let Some(action) = self.group_entry_panel.confirm.clone() {
                    ui.add_space(12.0);
                    egui::Frame::none()
                        .fill(pal.bg_elevated)
                        .rounding(Rounding::same(10.0))
                        .stroke(Stroke::new(1.2_f32, pal.warning))
                        .inner_margin(Margin::same(14.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(match &action {
                                    crate::group_entry::Action::RejectInvitation {
                                        block_group: true,
                                        ..
                                    } => "Từ chối và chặn lời mời từ nhóm này? Người mời không bị chặn.",
                                    crate::group_entry::Action::RejectInvitation { .. } => {
                                        "Từ chối lời mời nhóm này?"
                                    }
                                    _ => "Gửi yêu cầu tham gia nhóm? Có thể cần quản trị viên duyệt.",
                                })
                                .size(13.0)
                                .color(pal.text_primary)
                                .strong(),
                            );
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add_enabled(
                                        !self.group_entry_panel.pending,
                                        egui::Button::new(
                                            RichText::new("Xác nhận").color(Color32::WHITE).strong(),
                                        )
                                        .fill(pal.primary),
                                    )
                                    .clicked()
                                {
                                    intent = Some(action);
                                    self.group_entry_panel.confirm = None;
                                }
                                if ui.button("Hủy").clicked() {
                                    self.group_entry_panel.confirm = None;
                                }
                            });
                        });
                }

                if self.group_entry_panel.uncertain {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(
                            "Chưa rõ kết quả yêu cầu. Tải lại lời mời hoặc kiểm tra trên Zalo chính thức trước khi thử tiếp.",
                        )
                        .size(12.0)
                        .color(pal.warning),
                    );
                }
            });

        if let Some(action) = intent {
            self.group_entry_command(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thread(id: &str, name: &str, is_group: bool) -> Thread {
        Thread {
            id: id.into(),
            name: name.into(),
            is_group,
            presentation: Default::default(),
            unread: Default::default(),
            preview: String::new(),
            time: String::new(),
            avatar: None,
            messages: Vec::new(),
        }
    }

    #[test]
    fn sections_group_by_folded_initial_and_put_symbols_last() {
        let threads = vec![
            thread("1", "Đức", false),
            thread("2", "an", false),
            thread("3", "Ánh", false),
            thread("4", "123", false),
            thread("5", "Nhóm A", true),
            thread("6", "Cloud của tôi", false),
        ];
        let sections = contact_sections(&threads, &HashMap::new(), ContactsTab::Friends, "");
        assert_eq!(
            sections,
            vec![('A', vec![1, 2]), ('D', vec![0]), ('#', vec![3])]
        );
        assert_eq!(
            contact_sections(&threads, &HashMap::new(), ContactsTab::Groups, ""),
            vec![('N', vec![4])]
        );
    }

    #[test]
    fn sections_skip_hidden_and_match_query_without_diacritics() {
        let threads = vec![
            thread("1", "Nguyễn Văn An", false),
            thread("2", "Bình", false),
        ];
        let preferences = HashMap::from([(
            "2".to_string(),
            crate::store::ConversationPreference {
                hidden: true,
                ..Default::default()
            },
        )]);
        assert_eq!(
            contact_sections(&threads, &preferences, ContactsTab::Friends, ""),
            vec![('N', vec![0])]
        );
        assert_eq!(
            contact_sections(&threads, &HashMap::new(), ContactsTab::Friends, "van an"),
            vec![('N', vec![0])]
        );
        assert!(contact_sections(&threads, &HashMap::new(), ContactsTab::Friends, "zz").is_empty());
    }

    #[test]
    fn test_contacts_tab_variants_and_navigation() {
        assert_eq!(ContactsTab::default(), ContactsTab::Friends);

        let tabs = [
            ContactsTab::Friends,
            ContactsTab::Groups,
            ContactsTab::FriendRequests,
            ContactsTab::GroupInvitations,
        ];
        assert_eq!(tabs.len(), 4);

        let mut current = ContactsTab::default();
        assert_eq!(current, ContactsTab::Friends);
        current = ContactsTab::Groups;
        assert_eq!(current, ContactsTab::Groups);
        current = ContactsTab::FriendRequests;
        assert_eq!(current, ContactsTab::FriendRequests);
        current = ContactsTab::GroupInvitations;
        assert_eq!(current, ContactsTab::GroupInvitations);
    }

    #[test]
    fn test_friend_requests_and_group_invitations_sections() {
        let threads = vec![
            thread("100", "Lê Văn C", false),
            thread("200", "Nhóm Thể Thao", true),
        ];
        // Friends tab only includes non-groups
        let friend_sections = contact_sections(&threads, &HashMap::new(), ContactsTab::Friends, "");
        assert_eq!(friend_sections, vec![('L', vec![0])]);

        // Groups tab only includes groups
        let group_sections = contact_sections(&threads, &HashMap::new(), ContactsTab::Groups, "");
        assert_eq!(group_sections, vec![('N', vec![1])]);
    }

    #[test]
    fn contact_sections_prioritize_saved_phonebook_aliases_and_match_both_names() {
        let mut t1 = thread("10", "Zalo User", false);
        t1.presentation.base_name = Some("Zalo User".into());
        let t2 = thread("20", "Anh Bình", false);
        let grp = thread("10", "Nhóm Dự Án", true);
        let threads = vec![t1, t2, grp];
        let aliases = HashMap::from([
            ("10".to_string(), "Bố Yêu".to_string()),
            ("20".to_string(), "   ".to_string()),
        ]);

        // "10" sorts under 'B' after "Anh Bình" under 'A' when using alias "Bố Yêu", while empty alias falls back to "Anh Bình".
        assert_eq!(
            contact_sections_with_aliases(
                &threads,
                &HashMap::new(),
                &aliases,
                ContactsTab::Friends,
                "",
            ),
            vec![('A', vec![1]), ('B', vec![0])]
        );
        // Searching either the phonebook alias or original Zalo base_name finds the contact.
        assert_eq!(
            contact_sections_with_aliases(
                &threads,
                &HashMap::new(),
                &aliases,
                ContactsTab::Friends,
                "bo yeu",
            ),
            vec![('B', vec![0])]
        );
        assert_eq!(
            contact_sections_with_aliases(
                &threads,
                &HashMap::new(),
                &aliases,
                ContactsTab::Friends,
                "zalo user",
            ),
            vec![('B', vec![0])]
        );
        // Group threads never inherit a user alias on the same numeric ID.
        assert_eq!(
            contact_sections_with_aliases(
                &threads,
                &HashMap::new(),
                &aliases,
                ContactsTab::Groups,
                "",
            ),
            vec![('N', vec![2])]
        );
    }
}
