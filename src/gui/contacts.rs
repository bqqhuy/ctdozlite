//! Contacts page: friends and groups from loaded conversations, grouped A–Z.
//!
//! Lists only identities the client already holds (friend list and conversations); it shows
//! Group invitations and public username lookup use separate account-scoped panels.

use super::*;

/// Contacts page section.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ContactsTab {
    #[default]
    Friends,
    Groups,
}

/// Group thread indices into alphabetical sections by diacritic-folded name.
///
/// Excludes Cloud, hidden conversations and threads of the other kind. Letters outside A–Z
/// share the `#` section, which sorts last.
pub(super) fn contact_sections(
    threads: &[Thread],
    preferences: &HashMap<String, crate::store::ConversationPreference>,
    tab: ContactsTab,
    query: &str,
) -> Vec<(char, Vec<usize>)> {
    let query = fold_vn_search(query.trim());
    let mut entries: Vec<(String, usize)> = threads
        .iter()
        .enumerate()
        .filter(|(_, t)| t.is_group == (tab == ContactsTab::Groups))
        .filter(|(_, t)| !t.id.is_empty() && !t.name.contains("Cloud của tôi"))
        .filter(|(_, t)| !preferences.get(&t.id).is_some_and(|p| p.hidden))
        .map(|(i, t)| (fold_vn_search(t.name.trim()), i))
        .filter(|(name, _)| query.is_empty() || name.contains(&query))
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

impl ZaloGuiApp {
    fn visible_contact_sections(&self, tab: ContactsTab, query: &str) -> Vec<(char, Vec<usize>)> {
        let mut sections =
            contact_sections(&self.threads, &self.conversation_preferences, tab, query);
        for (_, indices) in &mut sections {
            indices.retain(|index| {
                let thread = &self.threads[*index];
                self.private_access(&thread.id, thread.is_group)
                    && (thread.is_group
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
        let (friends, groups) = (count(ContactsTab::Friends), count(ContactsTab::Groups));

        egui::SidePanel::left("contacts_sidebar")
            .resizable(false)
            .exact_width(260.0)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .inner_margin(Margin::symmetric(12.0, 14.0)),
            )
            .show(ctx, |ui| {
                if ui.button("Tra cứu username / bạn thân…").clicked() {
                    self.open_username_lookup();
                }
                ui.label(
                    RichText::new(crate::i18n::tr("Danh bạ"))
                        .size(16.0)
                        .strong()
                        .color(pal.text_primary),
                );
                if ui.button("Lời mời nhóm / Tham gia qua link…").clicked() {
                    self.open_group_entry();
                }
                if ui.button("Tạo nhóm").clicked() {
                    self.open_group_panel(String::new(), true, true);
                }
                ui.add_space(10.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.contacts_query)
                        .id(Id::new("contacts_search"))
                        .hint_text(crate::i18n::tr("Tìm theo tên"))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(12.0);
                for (tab, label, total) in [
                    (ContactsTab::Friends, "Bạn bè", friends),
                    (ContactsTab::Groups, "Nhóm", groups),
                ] {
                    let active = self.contacts_tab == tab;
                    let (rect, response) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                    if active {
                        ui.painter().rect_filled(
                            rect,
                            Rounding::same(8.0),
                            pal.primary.linear_multiply(0.18),
                        );
                    } else if response.hovered() {
                        ui.painter()
                            .rect_filled(rect, Rounding::same(8.0), pal.bg_hover);
                    }
                    ui.painter().text(
                        Pos2::new(rect.left() + 12.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("{label} ({total})"),
                        FontId::proportional(13.5),
                        if active {
                            pal.primary_hover
                        } else {
                            pal.text_primary
                        },
                    );
                    if response.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                        self.contacts_tab = tab;
                    }
                    ui.add_space(4.0);
                }
            });

        let sections = self.visible_contact_sections(self.contacts_tab, &self.contacts_query);
        let mut open: Option<usize> = None;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_chat_canvas)
                    .inner_margin(Margin::symmetric(24.0, 18.0)),
            )
            .show(ctx, |ui| {
                let total: usize = sections.iter().map(|(_, items)| items.len()).sum();
                ui.label(
                    RichText::new(match self.contacts_tab {
                        ContactsTab::Friends => format!("Bạn bè ({total})"),
                        ContactsTab::Groups => format!("Nhóm ({total})"),
                    })
                    .size(15.0)
                    .strong()
                    .color(pal.text_primary),
                );
                ui.add_space(10.0);
                if sections.is_empty() {
                    ui.label(
                        RichText::new(if self.contacts_query.trim().is_empty() {
                            "Chưa có liên hệ nào được tải."
                        } else {
                            "Không tìm thấy liên hệ phù hợp."
                        })
                        .color(pal.text_muted),
                    );
                    return;
                }
                egui::ScrollArea::vertical()
                    .id_salt(("contacts_list", self.contacts_tab as u8))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (letter, items) in &sections {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(letter.to_string())
                                    .size(13.0)
                                    .strong()
                                    .color(pal.text_secondary),
                            );
                            for &index in items {
                                let Some(thread) = self.threads.get(index) else {
                                    continue;
                                };
                                let (rect, response) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), 48.0),
                                    Sense::click(),
                                );
                                if response.hovered() {
                                    ui.painter().rect_filled(
                                        rect,
                                        Rounding::same(8.0),
                                        pal.bg_hover,
                                    );
                                }
                                let center = Pos2::new(rect.left() + 26.0, rect.center().y);
                                ui.painter().circle_filled(
                                    center,
                                    17.0,
                                    avatar_color_for(&thread.name, thread.is_group),
                                );
                                ui.painter().text(
                                    center,
                                    egui::Align2::CENTER_CENTER,
                                    avatar_initials(&thread.name),
                                    FontId::proportional(12.5),
                                    Color32::WHITE,
                                );
                                ui.painter().text(
                                    Pos2::new(rect.left() + 54.0, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &thread.name,
                                    FontId::proportional(14.0),
                                    pal.text_primary,
                                );
                                if response
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Mở hội thoại"))
                                    .clicked()
                                {
                                    open = Some(index);
                                }
                            }
                        }
                    });
            });
        if let Some(index) = open {
            if let Some(thread) = self.threads.get(index) {
                let (id, is_group) = (thread.id.clone(), thread.is_group);
                self.contacts_view_open = false;
                self.open_thread(ctx, index, id, is_group);
            }
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
}
