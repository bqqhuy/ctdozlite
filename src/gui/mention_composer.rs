//! Identity-preserving member suggestions; all roster I/O stays in the worker.
use super::*;

impl ZaloGuiApp {
    pub(super) fn mention_suggestions(
        &mut self,
        ui: &mut egui::Ui,
        thread: &str,
        cursor: usize,
        editor: &egui::Response,
        enter: &mut bool,
        ime: bool,
    ) {
        self.composer_mentions.edit(&self.input_message);
        let is_group = self.threads.iter().any(|t| t.id == thread && t.is_group);
        if !is_group {
            return;
        }
        if ime {
            *enter = false;
            return;
        }
        let Some((start, end, query)) = crate::mentions::query(&self.input_message, cursor) else {
            return;
        };
        let dismissed_key = Id::new(("member_suggest_dismissed", thread));
        let query_identity = (start, end, query.to_owned());
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            ui.ctx()
                .data_mut(|d| d.insert_temp(dismissed_key, query_identity));
            *enter = false;
            return;
        }
        if ui
            .ctx()
            .data(|d| d.get_temp::<(usize, usize, String)>(dismissed_key))
            .as_ref()
            == Some(&query_identity)
        {
            return;
        }
        let popup_key = Id::new(("member_suggest_rect", thread));
        let pointer_over_popup = ui
            .ctx()
            .data(|d| d.get_temp::<egui::Rect>(popup_key))
            .is_some_and(|rect| {
                ui.input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)))
            });
        if !editor.has_focus() && !pointer_over_popup {
            return;
        }
        let pos = self.input_message[..start].encode_utf16().count();
        if self.composer_mentions.mentions.iter().any(|m| m.pos == pos) {
            return;
        }
        let query = query.to_lowercase();
        let mut members = self
            .group_members
            .get(thread)
            .map(|(m, _)| m.clone())
            .or_else(|| {
                self.threads
                    .iter()
                    .find(|t| t.id == thread)
                    .map(|t| t.presentation.members.clone())
            })
            .unwrap_or_default();
        if !self.group_members_state.contains_key(thread)
            && !self.group_members.contains_key(thread)
        {
            if let Some(tx) = WORKER_TX.get() {
                if tx
                    .send(WorkerCmd::FetchGroupMembers {
                        group_id: thread.into(),
                    })
                    .is_ok()
                {
                    self.group_members_state.insert(thread.into(), None);
                }
            }
        }
        members.insert(
            0,
            crate::chat::SidebarMember {
                id: "-1".into(),
                name: "All".into(),
                ..Default::default()
            },
        );
        members.retain(|m| {
            (m.id == "-1"
                || (!m.id.is_empty() && m.id != "0" && m.id.bytes().all(|b| b.is_ascii_digit())))
                && m.id != self.user_uin
                && (m.name.to_lowercase().contains(&query) || m.id == query)
        });
        members.truncate(8);
        let mut chosen = if *enter {
            members.first().cloned()
        } else {
            None
        };
        // Enter selects a suggestion, never also sends the newly inserted mention.
        *enter = false;
        let popup = egui::Area::new(Id::new(("member_suggest", thread)))
            .order(egui::Order::Foreground)
            .fixed_pos(editor.rect.left_top() - Vec2::new(0.0, 8.0))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(240.0);
                    ui.label(crate::i18n::tr("Nhắc đến · Enter chọn · Esc giữ dạng chữ"));
                    if members.is_empty() {
                        ui.label(
                            if matches!(self.group_members_state.get(thread), Some(None)) {
                                "Đang tải thành viên…"
                            } else {
                                "Không tìm thấy thành viên"
                            },
                        );
                        if matches!(self.group_members_state.get(thread), Some(Some(_)))
                            && ui.button(crate::i18n::tr("Tải lại thành viên")).clicked()
                        {
                            if let Some(tx) = WORKER_TX.get() {
                                if tx
                                    .send(WorkerCmd::FetchGroupMembers {
                                        group_id: thread.into(),
                                    })
                                    .is_ok()
                                {
                                    self.group_members_state.insert(thread.into(), None);
                                }
                            }
                        }
                    }
                    for m in &members {
                        let label = if m.id == "-1" {
                            "@All · Tất cả thành viên".into()
                        } else {
                            format!("{} · {}", m.name, m.id)
                        };
                        if ui.button(label).clicked() {
                            chosen = Some(m.clone());
                        }
                    }
                });
            });
        ui.ctx()
            .data_mut(|d| d.insert_temp(popup_key, popup.response.rect));
        if let Some(m) = chosen {
            let old_len = self.input_message.len();
            self.composer_mentions.insert(start, end, &m.id, &m.name);
            self.input_message = self.composer_mentions.text.clone();
            editor.request_focus();
            let inserted = self
                .input_message
                .len()
                .saturating_sub(old_len - (end - start));
            let character = self.input_message[..start + inserted].chars().count();
            if let Some(mut state) = egui::text_edit::TextEditState::load(ui.ctx(), editor.id) {
                state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(
                        egui::text::CCursor::new(character),
                    )));
                state.store(ui.ctx(), editor.id);
            }
        }
    }
}

/// Selected identities have distinct styling; a typed or edited @name stays plain text.
pub(super) fn mention_layout(
    text: &str,
    saved: &crate::mentions::Draft,
    width: f32,
    pal: &ThemePalette,
) -> egui::text::LayoutJob {
    let mut draft = saved.clone();
    draft.edit(text);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = width;
    let plain = egui::text::TextFormat {
        font_id: FontId::proportional(14.0),
        color: pal.text_primary,
        ..Default::default()
    };
    let selected = egui::text::TextFormat {
        color: pal.primary,
        underline: Stroke::new(1.0_f32, pal.primary),
        ..plain.clone()
    };
    let mut offset = 0;
    for m in &draft.mentions {
        let Some(start) = crate::mentions::byte_at(text, m.pos) else {
            continue;
        };
        let Some(end) = crate::mentions::byte_at(text, m.pos.saturating_add(m.len)) else {
            continue;
        };
        if start < offset || end < start {
            continue;
        }
        job.append(&text[offset..start], 0.0, plain.clone());
        job.append(&text[start..end], 0.0, selected.clone());
        offset = end;
    }
    job.append(&text[offset..], 0.0, plain);
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enter_selects_a_real_uid_without_sending_and_ime_blocks_selection() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "group_1".into(),
                is_group: true,
                ..Default::default()
            }],
            false,
        );
        app.group_members.insert(
            "group_1".into(),
            (
                vec![crate::chat::SidebarMember {
                    id: "42".into(),
                    name: "An".into(),
                    ..Default::default()
                }],
                None,
            ),
        );
        app.input_message = "@An".into();
        let mut enter = true;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let editor = ui.text_edit_multiline(&mut app.input_message);
                editor.request_focus();
                app.mention_suggestions(ui, "group_1", 3, &editor, &mut enter, false);
            });
        });
        assert!(!enter);
        assert_eq!(app.input_message, "@An ");
        assert_eq!(app.composer_mentions.mentions[0].uid, "42");
        app.input_message = "@All".into();
        enter = true;
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let editor = ui.text_edit_multiline(&mut app.input_message);
                editor.request_focus();
                app.mention_suggestions(ui, "group_1", 4, &editor, &mut enter, true);
            });
        });
        assert!(!enter);
        assert_eq!(app.input_message, "@All");
        assert!(app.composer_mentions.mentions.is_empty());
    }
}
