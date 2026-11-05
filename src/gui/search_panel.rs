//! Debounced account-scoped search and conversation library; SQLite stays on the worker.
use super::*;
use crate::search::{Category, Page, Request, Sender};

#[derive(Default)]
pub(super) struct SearchPanel {
    pub open: bool,
    pub request: Request,
    pub page: Page,
    pub senders: Vec<Sender>,
    pub pending: bool,
    pub inflight: Option<u64>,
    pub due: Option<std::time::Instant>,
    pub status: String,
    pub from: String,
    pub to: String,
    pub opening: Option<(u64, String, String)>,
}
impl ZaloGuiApp {
    pub(super) fn open_history_search(
        &mut self,
        thread: Option<String>,
        category: Category,
        query: String,
    ) {
        let generation = crate::search::generation();
        self.history_search = SearchPanel {
            open: true,
            request: Request {
                generation,
                thread,
                category,
                query,
                hidden: self.show_hidden_conversations,
                ..Default::default()
            },
            due: Some(std::time::Instant::now()),
            ..Default::default()
        };
    }
    pub(super) fn invalidate_history_search(&mut self) {
        self.history_search.request.generation = crate::search::generation();
        self.history_search.request.cursor = None;
        self.history_search.page = Default::default();
        self.history_search.opening = None;
        if self.history_search.open {
            self.history_search.due = Some(std::time::Instant::now() + Duration::from_millis(250));
        }
    }
    pub(super) fn render_history_search(&mut self, ctx: &egui::Context) {
        if !self.is_logged_in {
            self.history_search = Default::default();
            return;
        }
        if !self.history_search.open {
            return;
        }
        let mut open = true;
        let mut changed = self.history_search.request.hidden != self.show_hidden_conversations;
        if changed {
            self.history_search.request.hidden = self.show_hidden_conversations;
            self.history_search.page = Default::default();
        }
        let mut more = false;
        let mut action = None;
        let mut media_action: Option<(String, bool, ChatMessage, bool)> = None;
        let library = self.history_search.request.thread.is_some();
        egui::Window::new(crate::i18n::tr(if library {
            "Tìm kiếm & thư viện hội thoại"
        } else {
            "Tìm kiếm toàn bộ lịch sử"
        }))
        .id(Id::new("history-search"))
        .open(&mut open)
        .default_width(760.0)
        .max_width((ctx.screen_rect().width()-32.0).max(240.0))
        .max_height((ctx.screen_rect().height() - 80.0).max(200.0))
        .show(ctx, |ui| {
            ui.label(crate::i18n::tr(
                "Dữ liệu đã lưu trên máy; không bao gồm lịch sử chưa đồng bộ.",
            ));
            ui.horizontal_wrapped(|ui| {
                for (category, label) in [
                    (Category::All, "Tất cả"),
                    (Category::Contacts, "Liên hệ & hội thoại"),
                    (Category::Messages, "Tin nhắn"),
                    (Category::Visual, "Ảnh / Video"),
                    (Category::Files, "File"),
                    (Category::Links, "Link"),
                ] {
                    if library && category == Category::Contacts {
                        continue;
                    }
                    changed |= ui
                        .selectable_value(
                            &mut self.history_search.request.category,
                            category,
                            crate::i18n::tr(label),
                        )
                        .changed();
                }
            });
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.history_search.request.query)
                        .char_limit(512)
                        .hint_text(crate::i18n::tr(
                            "Từ khóa, tên file, tiêu đề hoặc địa chỉ link…",
                        ))
                        .desired_width(f32::INFINITY),
                )
                .changed();
            ui.horizontal_wrapped(|ui| {
                if self.history_search.request.category==Category::Files {
                    changed|=ui.add(egui::TextEdit::singleline(&mut self.history_search.request.extension).hint_text(crate::i18n::tr("Đuôi tệp (pdf, xlsx…)")).char_limit(16).desired_width(130.0)).changed();
                }else if !self.history_search.request.extension.is_empty(){self.history_search.request.extension.clear();changed=true;}
                if library {
                    let current = self
                        .history_search
                        .senders
                        .iter()
                        .find(|s| s.id == self.history_search.request.sender)
                        .map(|s| s.name.as_str())
                        .unwrap_or("Tất cả người gửi");
                    egui::ComboBox::from_id_salt("search-sender")
                        .selected_text(current)
                        .show_ui(ui, |ui| {
                            changed |= ui
                                .selectable_value(
                                    &mut self.history_search.request.sender,
                                    String::new(),
                                    crate::i18n::tr("Tất cả người gửi"),
                                )
                                .changed();
                            for sender in &self.history_search.senders {
                                changed |= ui
                                    .selectable_value(
                                        &mut self.history_search.request.sender,
                                        sender.id.clone(),
                                        &sender.name,
                                    )
                                    .changed();
                            }
                        });
                }
                ui.label(crate::i18n::tr("Từ ngày"));
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut self.history_search.from)
                            .hint_text("YYYY-MM-DD")
                            .desired_width(100.0),
                    )
                    .changed();
                ui.label(crate::i18n::tr("Đến ngày"));
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut self.history_search.to)
                            .hint_text("YYYY-MM-DD")
                            .desired_width(100.0),
                    )
                    .changed();
                if ui.button(crate::i18n::tr("Làm mới")).clicked() {
                    changed = true;
                }
            });
            if self.history_search.pending {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(crate::i18n::tr("Đang tìm trong cơ sở dữ liệu…"));
                    ui.small(crate::i18n::tr("Lần tìm đầu sau nâng cấp cần chuẩn bị chỉ mục; tin nhắn vẫn được giữ nguyên."));
                });
            }
            if !self.history_search.status.is_empty() {
                ui.label(&self.history_search.status);
            }
            egui::ScrollArea::vertical()
                .max_height((ctx.screen_rect().height() - 310.0).max(120.0))
                .show(ui, |ui| {
                    for hit in &self.history_search.page.hits {
                        ui.push_id((&hit.thread, &hit.id), |ui| {
                            ui.group(|ui| {
                                ui.strong(if hit.title.is_empty() {
                                    &hit.thread_name
                                } else {
                                    &hit.title
                                });
                                ui.small(format!(
                                    "{} · {} · {} · {}",
                                    hit.thread_name,
                                    hit.sender,
                                    crate::clock::message_time(hit.ts),
                                    hit.kind
                                ));
                                ui.label(&hit.text);
                                if let Some(message) = hit.message.as_deref() {
                                    let media = crate::message_view::media(message);
                                    if hit.kind == "visual" {
                                        let key = crate::media_recovery::image_key(
                                            &hit.thread,
                                            &hit.id,
                                            0,
                                            false,
                                        );
                                        let (rect, _) = ui.allocate_exact_size(
                                            Vec2::new(128.0, 96.0),
                                            Sense::hover(),
                                        );
                                        if let Some(texture) = self.sticker_textures.get(&key) {
                                            let size = texture.size_vec2();
                                            let ratio =
                                                (rect.width() / size.x).min(rect.height() / size.y);
                                            let target =
                                                Rect::from_center_size(rect.center(), size * ratio);
                                            ui.painter().image(
                                                texture.id(),
                                                target,
                                                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                                                Color32::WHITE,
                                            );
                                        } else {
                                            ui.painter().rect_filled(
                                                rect,
                                                Rounding::same(8.0),
                                                ui.visuals().faint_bg_color,
                                            );
                                            if ui.is_rect_visible(rect)
                                                && !self.image_failures.contains_key(&key)
                                                && !self.sticker_textures.contains_key(&key)
                                            {
                                                if let Some(url) = media
                                                    .thumbnail
                                                    .as_deref()
                                                    .filter(|s| !s.is_empty())
                                                {
                                                    if self.loading_stickers.insert(key.clone()) {
                                                        if let Some(tx) = WORKER_TX.get() {
                                                            let _ = tx.send(
                                                                WorkerCmd::FetchChatImage {
                                                                    key: key.clone(),
                                                                    url: url.into(),
                                                                    thread: hit.thread.clone(),
                                                                    group: hit.group,
                                                                    message: Box::new(
                                                                        message.clone(),
                                                                    ),
                                                                    preview: false,
                                                                },
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                            if self.image_failures.contains_key(&key) {
                                                ui.small(crate::i18n::tr("Chưa có thumbnail"));
                                                if ui
                                                    .small_button(crate::i18n::tr("Thử tải lại"))
                                                    .clicked()
                                                {
                                                    self.image_failures.remove(&key);
                                                    self.loading_stickers.remove(&key);
                                                }
                                            }
                                        }
                                    }
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button(crate::i18n::tr(if hit.kind == "link" {
                                                "Mở liên kết"
                                            } else {
                                                "Mở tệp"
                                            }))
                                            .clicked()
                                        {
                                            media_action = Some((
                                                hit.thread.clone(),
                                                hit.group,
                                                message.clone(),
                                                false,
                                            ));
                                        }
                                        if hit.kind != "link"
                                            && ui.button(crate::i18n::tr("Lưu thành…")).clicked()
                                        {
                                            media_action = Some((
                                                hit.thread.clone(),
                                                hit.group,
                                                message.clone(),
                                                true,
                                            ));
                                        }
                                    });
                                }
                                if ui
                                    .button(if hit.id.is_empty() {
                                        "Mở hội thoại"
                                    } else {
                                        "Đi tới tin gốc"
                                    })
                                    .clicked()
                                {
                                    action = Some((hit.thread.clone(), hit.id.clone()));
                                }
                            });
                        });
                    }
                    if !self.history_search.pending
                        && self.history_search.page.hits.is_empty()
                        && self.history_search.due.is_none()
                    {
                        ui.label(crate::i18n::tr(
                            "Không có kết quả phù hợp trong dữ liệu đã lưu.",
                        ));
                    }
                    if self.history_search.page.next.is_some()
                        && ui
                            .add_enabled(
                                !self.history_search.pending,
                                egui::Button::new(crate::i18n::tr("Trang tiếp theo")),
                            )
                            .clicked()
                    {
                        more = true;
                    }
                });
        });
        if let Some((thread, group, message, save)) = media_action {
            if !self.threads.iter().any(|t| t.id == thread) {
                self.threads.push(Thread {
                    id: thread.clone(),
                    name: thread.clone(),
                    is_group: group,
                    ..Default::default()
                });
            }
            let media = crate::message_view::media(&message);
            if matches!(
                media.kind,
                crate::message_view::MediaKind::Photo | crate::message_view::MediaKind::Video
            ) && !save
            {
                self.open_media_viewer(&thread, &message);
                if media.kind == crate::message_view::MediaKind::Video {
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::DownloadMedia {
                            url: media.original.unwrap_or_default(),
                            name: media.name,
                            play: true,
                            open: false,
                            context: Some(super::worker::ChatImage {
                                thread,
                                group,
                                message,
                                preview: false,
                            }),
                        });
                    }
                }
            } else if media.kind != crate::message_view::MediaKind::None {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::DownloadMedia {
                        url: media.original.or(media.preview).unwrap_or_default(),
                        name: media.name,
                        play: false,
                        open: !save,
                        context: Some(super::worker::ChatImage {
                            thread,
                            group,
                            message,
                            preview: false,
                        }),
                    });
                }
            } else if let Some(url) = message
                .link_url
                .clone()
                .or_else(|| crate::chat::extract_first_url(&message.text))
            {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::OpenLibraryLink {
                        owner: self.user_uin.clone(),
                        url,
                    });
                }
            } else {
                self.history_search.status = "Thiếu thông tin mở tệp hoặc liên kết".into();
            }
        }
        self.history_search.open = open;
        if !open {
            self.history_search.request.generation = crate::search::generation();
            self.history_search.opening = None;
            return;
        }
        if changed {
            self.history_search.request.generation = crate::search::generation();
            self.history_search.request.cursor = None;
            self.history_search.opening = None;
            self.history_search.page = Default::default();
            self.history_search.due = Some(std::time::Instant::now() + Duration::from_millis(250));
        }
        if more {
            self.history_search.request.cursor = self.history_search.page.next.clone();
            self.history_search.request.generation = crate::search::generation();
            self.history_search.due = Some(std::time::Instant::now());
        }
        if let Some(due) = self.history_search.due {
            if due <= std::time::Instant::now() && !self.history_search.pending {
                let dates = date_bounds(&self.history_search.from, &self.history_search.to);
                match dates {
                    Ok((from, to)) => {
                        self.history_search.request.from = from;
                        self.history_search.request.to = to;
                        self.history_search.request.hidden = self.show_hidden_conversations;
                        self.history_search.pending = true;
                        self.history_search.inflight = Some(self.history_search.request.generation);
                        self.history_search.status.clear();
                        let sent = WORKER_TX.get().is_some_and(|tx| {
                            tx.send(WorkerCmd::Tools {
                                owner: self.user_uin.clone(),
                                command: super::tools_panel::ToolsCommand::SearchPage(
                                    self.history_search.request.clone(),
                                ),
                            })
                            .is_ok()
                        });
                        if !sent {
                            self.history_search.pending = false;
                            self.history_search.inflight = None;
                            self.history_search.status =
                                "Worker đã đóng; chưa thực hiện tìm kiếm".into();
                        }
                    }
                    Err(error) => self.history_search.status = error,
                }
                self.history_search.due = None;
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some((thread, id)) = action {
            if id.is_empty() {
                let index = self
                    .threads
                    .iter()
                    .position(|t| t.id == thread)
                    .or_else(|| {
                        let hit = self
                            .history_search
                            .page
                            .hits
                            .iter()
                            .find(|h| h.thread == thread)?;
                        self.threads.push(Thread {
                            id: thread.clone(),
                            name: hit.thread_name.clone(),
                            is_group: hit.group,
                            ..Default::default()
                        });
                        Some(self.threads.len() - 1)
                    });
                if let Some(index) = index {
                    let group = self.threads[index].is_group;
                    self.open_thread(ctx, index, thread.clone(), group);
                    self.history_search.open = false;
                    if !group {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                group_id: thread,
                                is_group: false,
                                limit: 150,
                            });
                        }
                    }
                }
            } else {
                let generation = self.history_search.request.generation;
                self.history_search.opening = Some((generation, thread.clone(), id.clone()));
                let sent = WORKER_TX.get().is_some_and(|tx| {
                    tx.send(WorkerCmd::Tools {
                        owner: self.user_uin.clone(),
                        command: super::tools_panel::ToolsCommand::OpenSearchHit {
                            generation,
                            thread,
                            id,
                        },
                    })
                    .is_ok()
                });
                if !sent {
                    self.history_search.opening = None;
                    self.history_search.status = "Worker đã đóng; chưa mở được kết quả".into();
                }
            }
        }
    }
}
fn date_bounds(from: &str, to: &str) -> Result<(i64, i64), String> {
    use chrono::{Local, NaiveDate, TimeZone};
    fn parse(text: &str, end: bool) -> Result<i64, String> {
        if text.trim().is_empty() {
            return Ok(0);
        }
        let day = NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d")
            .map_err(|_| "Ngày phải có dạng YYYY-MM-DD".to_owned())?;
        let time = if end {
            day.succ_opt().ok_or("Ngày không hợp lệ")?
        } else {
            day
        }
        .and_hms_opt(0, 0, 0)
        .ok_or("Ngày không hợp lệ")?;
        let ts = Local
            .from_local_datetime(&time)
            .earliest()
            .ok_or("Mốc giờ không hợp lệ tại múi giờ hiện tại")?
            .timestamp_millis();
        Ok(if end { ts - 1 } else { ts })
    }
    let (from, to) = (parse(from, false)?, parse(to, true)?);
    if from < 0 || to < 0 || (to > 0 && to < from) {
        return Err("Khoảng thời gian không hợp lệ".into());
    }
    Ok((from, to))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignores_stale_search_pages_after_reopening_or_account_switch() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "owner-1".into();
        app.open_history_search(None, Category::Messages, "first".into());
        let old = app.history_search.request.clone();
        app.open_history_search(Some("thread-2".into()), Category::Files, "second".into());
        let current = app.history_search.request.clone();
        app.history_search.pending = true;
        app.history_search.inflight = Some(current.generation);
        let (tx, rx) = channel();
        app.event_rx = rx;
        tx.send(GuiEvent::Tools {
            owner: "owner-1".into(),
            result: Ok(super::super::tools_panel::ToolsResult::SearchPage {
                request: old,
                result: Err("stale".into()),
                senders: Vec::new(),
            }),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.history_search.pending);
        assert!(app.history_search.status.is_empty());
        tx.send(GuiEvent::Tools {
            owner: "other-owner".into(),
            result: Ok(super::super::tools_panel::ToolsResult::SearchPage {
                request: current.clone(),
                result: Err("wrong account".into()),
                senders: Vec::new(),
            }),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.history_search.pending);
        tx.send(GuiEvent::Tools {
            owner: "owner-1".into(),
            result: Ok(super::super::tools_panel::ToolsResult::SearchPage {
                request: current,
                result: Ok(Page::default()),
                senders: Vec::new(),
            }),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(!app.history_search.pending);
        assert!(app.history_search.status.is_empty());
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    #[test]
    fn renders_library_and_server_board_with_unicode_at_small_and_large_sizes() {
        for (width, height) in [(480.0, 480.0), (1280.0, 800.0)] {
            for english in [false, true] {
                let ctx = egui::Context::default();
                let mut app = ZaloGuiApp::from_context(
                    &ctx,
                    Session::new(false, false, false).unwrap(),
                    false,
                    Vec::new(),
                    false,
                );
                app.is_logged_in = true;
                app.user_uin = "test-owner".into();
                crate::i18n::set_language(if english { "English" } else { "Tiếng Việt" });
                app.open_history_search(
                    Some("thread-test".into()),
                    Category::Files,
                    "Nguyễn 😀 %_".into(),
                );
                app.history_search.due = None;
                app.history_search.page.hits.push(crate::search::Hit {
                    thread: "thread-test".into(),
                    id: "message-test".into(),
                    ts: 1000,
                    thread_name: "Nhóm thử".into(),
                    sender: "Tên trùng".into(),
                    kind: "file".into(),
                    title: "Báo cáo.xlsx".into(),
                    text: "Kiểm tra chữ tổ hợp e\u{0302}".into(),
                    group: true,
                    message: None,
                });
                app.server_board = super::super::board_panel::BoardPanel {
                    open: true,
                    thread: "thread-test".into(),
                    group: true,
                    ..Default::default()
                };
                let frame = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(width, height),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        app.render_history_search(ctx);
                        app.render_server_board(ctx);
                    },
                );
                assert!(!frame.shapes.is_empty());
            }
        }
        crate::i18n::set_language("Tiếng Việt");
    }
}
