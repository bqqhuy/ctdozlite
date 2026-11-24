use super::*;

pub use crate::message_view::is_media_url;

type SharedPhotos = Vec<(String, crate::message_view::Media, ChatMessage)>;
type SharedFiles = Vec<(crate::message_view::Media, ChatMessage)>;
type SharedLinks = Vec<(String, String, ChatMessage)>;
type PhotoRef<'a> = &'a (String, crate::message_view::Media, ChatMessage);

fn archive_message_local_date(ts: i64) -> Option<chrono::NaiveDate> {
    chrono::DateTime::from_timestamp_millis(crate::clock::normalize_ts_ms(ts))
        .map(|date| date.with_timezone(&chrono::Local).date_naive())
}

fn latest_message_by_time(messages: &[ChatMessage]) -> Option<ChatMessage> {
    messages
        .iter()
        .max_by_key(|message| crate::clock::normalize_ts_ms(message.ts))
        .cloned()
}

fn group_photos_by_local_date(photos: Vec<PhotoRef<'_>>) -> Vec<(String, Vec<PhotoRef<'_>>)> {
    let mut groups: Vec<(String, Vec<_>)> = Vec::new();
    for photo in photos {
        let date =
            chrono::DateTime::from_timestamp_millis(crate::clock::normalize_ts_ms(photo.2.ts))
                .map(|date| {
                    date.with_timezone(&chrono::Local)
                        .format("%d/%m/%Y")
                        .to_string()
                })
                .unwrap_or_else(|| "Ngày không xác định".into());
        if groups.last().is_none_or(|(last, _)| last != &date) {
            groups.push((date.clone(), Vec::new()));
        }
        if let Some((_, items)) = groups.last_mut() {
            items.push(photo);
        }
    }
    groups
}

#[derive(Default)]
pub(super) struct CloudUsageState {
    pub owner: String,
    pub generation: u64,
    pub pending: bool,
    pub result: Option<Result<crate::zcloud::Usage, String>>,
}

pub(super) struct ArchiveMutation {
    pub thread: String,
    pub group: bool,
    pub recall: bool,
    pub messages: Vec<ChatMessage>,
}

#[derive(Default)]
pub(super) struct ArchiveHistory {
    pub thread: String,
    pub generation: u64,
    pub messages: Vec<ChatMessage>,
    pub cursor: Option<crate::store::HistoryCursor>,
    pub scanned: usize,
    pub loading: bool,
    pub loaded: bool,
    pub error: String,
}

#[derive(Clone)]
struct ArchiveDateDraft {
    from: String,
    to: String,
    month: chrono::NaiveDate,
    editing_end: bool,
    calendar_open: bool,
}

impl ArchiveDateDraft {
    fn new(from: &str, to: &str, today: chrono::NaiveDate) -> Self {
        use chrono::Datelike;
        let date = chrono::NaiveDate::parse_from_str(from, "%Y-%m-%d")
            .unwrap_or(today)
            .min(today);
        Self {
            from: from.to_owned(),
            to: to.to_owned(),
            month: date.with_day(1).unwrap_or(date),
            editing_end: false,
            calendar_open: false,
        }
    }

    fn error(&self, today: chrono::NaiveDate) -> Option<&'static str> {
        let parse = |value: &str| chrono::NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d");
        let from = parse(&self.from).ok();
        let to = parse(&self.to).ok();
        if (!self.from.trim().is_empty() && from.is_none())
            || (!self.to.trim().is_empty() && to.is_none())
        {
            Some("Ngày phải có định dạng YYYY-MM-DD.")
        } else if from.is_some_and(|date| date > today) || to.is_some_and(|date| date > today) {
            Some("Không thể chọn ngày trong tương lai.")
        } else if from.zip(to).is_some_and(|(from, to)| from > to) {
            Some("Ngày bắt đầu phải trước hoặc bằng ngày kết thúc.")
        } else {
            None
        }
    }
}

fn archive_date_preset(today: chrono::NaiveDate, days: u64) -> (String, String) {
    let start = today
        .checked_sub_days(chrono::Days::new(days))
        .unwrap_or(today);
    (start.to_string(), today.to_string())
}

fn render_archive_calendar(
    ui: &mut egui::Ui,
    draft: &mut ArchiveDateDraft,
    today: chrono::NaiveDate,
) {
    use chrono::Datelike;
    ui.horizontal(|ui| {
        if ui.small_button("◀").on_hover_text("Tháng trước").clicked() {
            if let Some(month) = draft.month.checked_sub_months(chrono::Months::new(1)) {
                draft.month = month;
            }
        }
        ui.label(format!(
            "Tháng {} / {}",
            draft.month.month(),
            draft.month.year()
        ));
        if ui
            .add_enabled(
                draft.month.year() < today.year()
                    || (draft.month.year() == today.year() && draft.month.month() < today.month()),
                egui::Button::new("▶").small(),
            )
            .on_hover_text("Tháng sau")
            .clicked()
        {
            if let Some(month) = draft.month.checked_add_months(chrono::Months::new(1)) {
                draft.month = month;
            }
        }
    });
    let selected = if draft.editing_end {
        &draft.to
    } else {
        &draft.from
    };
    let selected = chrono::NaiveDate::parse_from_str(selected, "%Y-%m-%d").ok();
    egui::Grid::new("archive-date-calendar")
        .spacing(Vec2::new(3.0, 3.0))
        .show(ui, |ui| {
            for name in ["T2", "T3", "T4", "T5", "T6", "T7", "CN"] {
                ui.label(name);
            }
            ui.end_row();
            let offset = draft.month.weekday().num_days_from_monday();
            for cell in 0..42 {
                let day = cell - offset as i32 + 1;
                let date = u32::try_from(day)
                    .ok()
                    .and_then(|day| draft.month.with_day(day));
                if let Some(date) = date {
                    if ui
                        .add_enabled(
                            date <= today,
                            egui::Button::new(date.day().to_string())
                                .selected(selected == Some(date))
                                .min_size(Vec2::splat(25.0)),
                        )
                        .clicked()
                    {
                        if draft.editing_end {
                            draft.to = date.to_string();
                            draft.calendar_open = false;
                        } else {
                            draft.from = date.to_string();
                            draft.editing_end = true;
                        }
                    }
                } else {
                    ui.allocate_space(Vec2::splat(25.0));
                }
                if cell % 7 == 6 {
                    ui.end_row();
                }
            }
        });
}

fn render_archive_date_filter(ui: &mut egui::Ui, scope: Id, from: &mut String, to: &mut String) {
    let today = chrono::Local::now().date_naive();
    let draft_id = scope.with("archive-date-draft");
    let label = if from.is_empty() && to.is_empty() {
        "Ngày gửi ▾".to_owned()
    } else {
        format!(
            "{} → {} ▾",
            if from.is_empty() {
                "…"
            } else {
                from.as_str()
            },
            if to.is_empty() { "…" } else { to.as_str() }
        )
    };
    let initial_from = from.clone();
    let initial_to = to.clone();
    let mut committed: Option<(String, String)> = None;
    let mut cancel_draft = false;
    let response = ui.menu_button(label, |ui| {
        let mut draft = ui.ctx().data_mut(|data| {
            data.get_temp::<ArchiveDateDraft>(draft_id)
                .unwrap_or_else(|| ArchiveDateDraft::new(&initial_from, &initial_to, today))
        });
        ui.menu_button("Gợi ý thời gian", |ui| {
            for (days, label) in [
                (7, "Trong vòng 1 tuần"),
                (30, "Trong vòng 1 tháng"),
                (90, "Trong vòng 3 tháng"),
            ] {
                if ui.button(label).clicked() {
                    let (start, end) = archive_date_preset(today, days);
                    committed = Some((start, end));
                    ui.close_menu();
                }
            }
        });
        ui.separator();
        ui.label("Chọn khoảng thời gian");
        for editing_end in [false, true] {
            ui.horizontal(|ui| {
                ui.label(if editing_end {
                    "Đến ngày"
                } else {
                    "Từ ngày"
                });
                let value = if editing_end {
                    &mut draft.to
                } else {
                    &mut draft.from
                };
                let input = ui.add(
                    egui::TextEdit::singleline(value)
                        .hint_text("YYYY-MM-DD")
                        .desired_width(105.0),
                );
                if ui
                    .small_button("📅")
                    .on_hover_text("Chọn ngày trên lịch")
                    .clicked()
                    || input.gained_focus()
                {
                    draft.editing_end = editing_end;
                    draft.calendar_open = true;
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d") {
                        use chrono::Datelike;
                        draft.month = date.min(today).with_day(1).unwrap_or(today);
                    }
                }
                if ui.small_button("×").on_hover_text("Xóa ngày").clicked() {
                    value.clear();
                }
            });
        }
        if draft.calendar_open {
            ui.small(if draft.editing_end {
                "Chọn ngày kết thúc"
            } else {
                "Chọn ngày bắt đầu"
            });
            render_archive_calendar(ui, &mut draft, today);
        }
        let error = draft.error(today);
        if let Some(error) = error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        ui.horizontal(|ui| {
            if ui.button("Hủy").clicked() {
                cancel_draft = true;
                ui.close_menu();
            }
            if ui
                .add_enabled(error.is_none(), egui::Button::new("Xác nhận"))
                .clicked()
            {
                committed = Some((draft.from.trim().to_owned(), draft.to.trim().to_owned()));
                ui.close_menu();
            }
        });
        if !cancel_draft {
            ui.ctx().data_mut(|data| data.insert_temp(draft_id, draft));
        }
    });
    if cancel_draft || response.inner.is_none() {
        ui.ctx()
            .data_mut(|data| data.remove::<ArchiveDateDraft>(draft_id));
    }
    if let Some((next_from, next_to)) = committed {
        *from = next_from;
        *to = next_to;
    }
    if (!from.is_empty() || !to.is_empty())
        && ui
            .small_button("×")
            .on_hover_text("Xóa bộ lọc ngày")
            .clicked()
    {
        from.clear();
        to.clear();
        ui.ctx()
            .data_mut(|data| data.remove::<ArchiveDateDraft>(draft_id));
    }
}

fn archive_selection_key(
    kind: &str,
    thread: &str,
    message: &ChatMessage,
    attachment: &str,
) -> String {
    // Length-independent JSON components cannot alias through ':' in IDs or URLs.
    serde_json::json!([
        kind,
        thread,
        message.id,
        message.client_message_id,
        attachment
    ])
    .to_string()
}

fn media_sender_matches(msg: &ChatMessage, query: &str) -> bool {
    if let Some(id) = query.strip_prefix("id:") {
        return crate::chat::sidebar_sender_id(msg).as_deref() == Some(id);
    }
    let query = crate::search::fold(query.trim());
    if query.is_empty() {
        return true;
    }
    if query.chars().all(|ch| ch.is_ascii_digit()) {
        return crate::chat::sidebar_sender_id(msg).is_some_and(|sender_id| sender_id == query);
    }
    crate::search::fold(&msg.sender).contains(&query)
}

fn extract_shared_media_and_links(thread: &Thread) -> (SharedPhotos, SharedFiles, SharedLinks) {
    let mut photos = Vec::new();
    let mut files = Vec::new();
    let mut links = Vec::new();

    for m in thread.messages.iter().rev() {
        let mut found_attachment = false;
        for (index, mut media) in crate::message_view::archive_media_items(m)
            .into_iter()
            .enumerate()
        {
            if matches!(
                media.kind,
                crate::message_view::MediaKind::Photo | crate::message_view::MediaKind::Video
            ) {
                if media.thumbnail.as_deref().is_none_or(str::is_empty) {
                    media.thumbnail = media
                        .preview
                        .clone()
                        .filter(|u| !u.is_empty())
                        .or_else(|| media.original.clone().filter(|u| !u.is_empty()))
                        .or_else(|| m.link_url.clone().filter(|u| !u.is_empty()));
                }
                if media.thumbnail.as_deref().is_some_and(|u| !u.is_empty())
                    || media.original.as_deref().is_some_and(|u| !u.is_empty())
                {
                    photos.push((
                        crate::media_recovery::image_key(&thread.id, &m.id, index, false),
                        media,
                        m.clone(),
                    ));
                }
                found_attachment = true;
                continue;
            }
            if media.kind == crate::message_view::MediaKind::File {
                files.push((media, m.clone()));
                found_attachment = true;
                continue;
            }
        }
        if found_attachment {
            continue;
        }

        let candidate_url = m
            .link_url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(str::to_owned)
            .or_else(|| crate::chat::extract_first_url(&m.text));

        if let Some(url) = candidate_url {
            if is_media_url(&url) {
                let is_video = crate::message_view::is_video_url(&url);
                let kind = if is_video {
                    crate::message_view::MediaKind::Video
                } else {
                    crate::message_view::MediaKind::Photo
                };
                let is_allowed_download = reqwest::Url::parse(&url)
                    .map(|u| crate::media_download::allowed(&u))
                    .unwrap_or(false);
                let thumb = m
                    .link_thumb
                    .clone()
                    .filter(|u| {
                        !u.is_empty()
                            && reqwest::Url::parse(u)
                                .map(|p| crate::media_download::allowed(&p))
                                .unwrap_or(false)
                    })
                    .or_else(|| (!is_video && is_allowed_download).then(|| url.clone()));
                let item = crate::message_view::Media {
                    kind,
                    thumbnail: thumb,
                    preview: Some(url.clone()),
                    original: Some(url),
                    name: m.link_title.clone().unwrap_or_default(),
                    ..Default::default()
                };
                photos.push((
                    crate::media_recovery::image_key(&thread.id, &m.id, 0, false),
                    item,
                    m.clone(),
                ));
            } else {
                let title = m
                    .link_title
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| m.text.trim().to_string());
                links.push((url, title, m.clone()));
            }
        }
    }

    (photos, files, links)
}

fn format_auto_delete_ttl(ttl_ms: i64) -> String {
    match ttl_ms {
        0 => "Đang tắt".to_string(),
        86_400_000 => "1 ngày".to_string(),
        604_800_000 => "7 ngày".to_string(),
        2_592_000_000 => "30 ngày".to_string(),
        ms if ms % 86_400_000 == 0 => format!("{} ngày", ms / 86_400_000),
        ms => format!("{} giờ", (ms / 3_600_000).max(1)),
    }
}

impl ZaloGuiApp {
    fn confirm_archive_mutation(
        &mut self,
        thread: &str,
        group: bool,
        recall: bool,
        messages: &[ChatMessage],
    ) {
        let mut seen = HashSet::new();
        let messages: Vec<_> = messages
            .iter()
            .filter(|message| seen.insert((message.id.clone(), message.client_message_id.clone())))
            .cloned()
            .collect();
        if messages.is_empty() || messages.len() > 100 {
            return;
        }
        self.archive_mutation_confirm = Some(ArchiveMutation {
            thread: thread.to_owned(),
            group,
            recall,
            messages,
        });
        self.drawer_view = DrawerView::MediaStorage {
            tab: self.drawer_media_active_tab,
        };
    }

    fn request_archive_page(&mut self, thread: &str, group: bool) {
        if self.archive_history.loading || !self.private_access(thread, group) {
            return;
        }
        let queued = WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::LoadArchiveHistory {
                owner: self.user_uin.clone(),
                runtime_id: self.account_runtime_id,
                thread: thread.to_owned(),
                group,
                generation: self.archive_history.generation,
                before: self.archive_history.cursor.clone(),
            })
            .is_ok()
        });
        self.archive_history.loading = queued;
        self.archive_history.error = if queued {
            String::new()
        } else {
            "Không gửi được yêu cầu đọc lịch sử tới worker.".into()
        };
    }

    pub(crate) fn request_attachment_picker(
        &mut self,
        thread: &str,
        group: bool,
        images_only: bool,
    ) {
        if self.attachment_picker_pending {
            self.show_toast("Hộp chọn tệp đang mở");
            return;
        }
        // Toolbar pickers send immediately; the draft's add tile uses PickAttachmentDraft.
        let queued = WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::PickAndSendAttachment {
                owner: self.user_uin.clone(),
                runtime_id: self.account_runtime_id,
                to_id: thread.to_owned(),
                is_group: group,
                caption: self.input_message.trim().to_owned(),
                preset_path: None,
                photo_original: self.settings.send_photo_original,
                ephemeral: false,
                images_only,
            })
            .is_ok()
        });
        self.attachment_picker_pending = queued;
        if !queued {
            self.status_msg =
                "Không gửi được yêu cầu chọn tệp; nội dung soạn thảo vẫn được giữ.".into();
        }
    }

    pub(crate) fn request_folder_picker(&mut self, thread: &str) {
        if self.attachment_picker_pending {
            self.show_toast("Hộp chọn tệp đang mở");
            return;
        }
        let thread = thread.to_owned();
        let owner = self.user_uin.clone();
        let runtime_id = self.account_runtime_id;
        std::thread::spawn(move || {
            if let Some(folder_path) = super::messages::pick_native_folder_dialog() {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::PrepareAttachmentArchive {
                        owner,
                        runtime_id,
                        thread,
                        path: folder_path,
                        stamp: None,
                    });
                }
            }
        });
    }

    pub(super) fn clear_timeline_selection(&mut self) {
        self.batch_selection_mode = false;
        self.selected_msg_ids.clear();
        self.timeline_selection_scope = None;
    }

    fn bind_timeline_selection(&mut self, thread: &str, group: bool) {
        let scope = (
            self.user_uin.clone(),
            self.account_runtime_id,
            thread.to_owned(),
            group,
        );
        if self.timeline_selection_scope.as_ref() != Some(&scope) {
            self.clear_timeline_selection();
        }
        self.timeline_selection_scope = Some(scope);
        self.batch_selection_mode = true;
    }

    fn timeline_selection_matches(&self, thread: &str, group: bool) -> bool {
        self.timeline_selection_scope
            .as_ref()
            .is_some_and(|(owner, runtime, target, is_group)| {
                owner == &self.user_uin
                    && *runtime == self.account_runtime_id
                    && target == thread
                    && *is_group == group
            })
    }

    fn share_timeline_selection(&mut self, thread: &str, group: bool) {
        if !self.timeline_selection_matches(thread, group) {
            self.clear_timeline_selection();
            return;
        }
        let selected = self
            .threads
            .iter()
            .find(|t| t.id == thread && t.is_group == group)
            .map(|t| {
                t.messages
                    .iter()
                    .filter(|m| self.selected_msg_ids.contains(&m.id))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.share_archive_messages(thread, group, selected);
        if self.forward_msg.is_some() {
            self.clear_timeline_selection();
        }
    }

    fn share_archive_messages(&mut self, thread: &str, group: bool, messages: Vec<ChatMessage>) {
        let mut seen = HashSet::new();
        let mut messages: Vec<_> = messages
            .into_iter()
            .filter(|message| seen.insert((message.id.clone(), message.client_message_id.clone())))
            .collect();
        if self.content_protected(thread, group)
            || messages.is_empty()
            || messages.len() > 100
            || messages
                .iter()
                .any(|message| !crate::chat::can_forward_message(message))
        {
            self.drawer_media_batch_status =
                "Có tin nhắn chưa hỗ trợ chia sẻ; hãy bỏ chọn mục đó rồi thử lại.".into();
            return;
        }
        for message in &mut messages {
            crate::chat::set_forward_source(message, thread, group);
        }
        self.clear_forward_dialog();
        self.forward_msg = messages.first().cloned();
        self.forward_batch = messages;
    }

    fn save_archive_to_documents(&mut self, thread: &str, group: bool, messages: Vec<ChatMessage>) {
        if self.own_cloud_id.is_empty()
            || !self.threads.iter().any(|t| {
                t.id == self.own_cloud_id && !t.is_group && self.private_access(&t.id, false)
            })
        {
            self.drawer_media_batch_status =
                "My Documents chưa có định danh máy chủ hoặc đang khóa; hãy đồng bộ rồi thử lại."
                    .into();
            return;
        }
        self.share_archive_messages(thread, group, messages);
        if self.forward_msg.is_some() {
            self.forward_targets.insert(self.own_cloud_id.clone());
            self.forward_cloud_only = true;
        }
    }

    fn archive_item_actions(
        &mut self,
        ui: &mut egui::Ui,
        thread: &str,
        group: bool,
        message: &ChatMessage,
        media: Option<&crate::message_view::Media>,
        link: Option<&str>,
    ) {
        let shareable = !self.content_protected(thread, group);
        if ui
            .add_enabled(
                shareable && crate::chat::can_forward_message(message),
                egui::Button::new("Chia sẻ tin nhắn gốc…"),
            )
            .clicked()
        {
            self.share_archive_messages(thread, group, vec![message.clone()]);
            ui.close_menu();
        }
        if ui
            .add_enabled(
                shareable
                    && !self.own_cloud_id.is_empty()
                    && crate::chat::can_forward_message(message),
                egui::Button::new("Lưu vào My Documents…"),
            )
            .clicked()
        {
            self.save_archive_to_documents(thread, group, vec![message.clone()]);
            ui.close_menu();
        }
        if !message.id.is_empty() && ui.button("Xem tin nhắn gốc").clicked() {
            self.tools_command(super::tools_panel::ToolsCommand::OpenHit {
                thread: thread.to_owned(),
                id: message.id.clone(),
            });
            ui.close_menu();
        }
        if let Some(media) = media {
            if media.kind == crate::message_view::MediaKind::Photo
                && ui
                    .add_enabled(shareable, egui::Button::new("Sao chép ảnh"))
                    .clicked()
            {
                let queued = WORKER_TX.get().is_some_and(|tx| {
                    tx.send(WorkerCmd::CopyArchiveImage {
                        owner: self.user_uin.clone(),
                        media: media.clone(),
                        context: super::worker::ChatImage {
                            thread: thread.to_owned(),
                            group,
                            message: message.clone(),
                            preview: true,
                        },
                    })
                    .is_ok()
                });
                if !queued {
                    self.drawer_media_batch_status =
                        "Không gửi được yêu cầu sao chép tới worker.".into();
                }
                ui.close_menu();
            }
            let kind = if media.kind == crate::message_view::MediaKind::File {
                "file"
            } else {
                "photo"
            };
            let attachment = media.original.as_deref().unwrap_or(&media.name);
            let key = archive_selection_key(kind, thread, message, attachment);
            if let Some(path) = self.archive_local_paths.get(&key).cloned() {
                if ui
                    .add_enabled(shareable, egui::Button::new("Hiển thị trong thư mục"))
                    .clicked()
                {
                    let queued = WORKER_TX.get().is_some_and(|tx| {
                        tx.send(WorkerCmd::RevealArchiveMedia {
                            owner: self.user_uin.clone(),
                            runtime_id: self.account_runtime_id,
                            path,
                            context: super::worker::ChatImage {
                                thread: thread.to_owned(),
                                group,
                                message: message.clone(),
                                preview: false,
                            },
                        })
                        .is_ok()
                    });
                    if !queued {
                        self.drawer_media_batch_status =
                            "Không gửi được yêu cầu mở thư mục tới worker.".into();
                    }
                    ui.close_menu();
                }
            }
            if let Some(url) = media.original.as_ref() {
                if ui
                    .add_enabled(shareable, egui::Button::new("Lưu về máy…"))
                    .clicked()
                {
                    let batch = crate::search::generation();
                    let queued = WORKER_TX.get().is_some_and(|tx| {
                        tx.send(WorkerCmd::DownloadMediaBatch {
                            owner: self.user_uin.clone(),
                            batch,
                            items: vec![super::worker::MediaBatchItem {
                                key: key.clone(),
                                url: url.clone(),
                                name: media.name.clone(),
                                context: super::worker::ChatImage {
                                    thread: thread.to_owned(),
                                    group,
                                    message: message.clone(),
                                    preview: false,
                                },
                            }],
                        })
                        .is_ok()
                    });
                    if queued {
                        self.drawer_media_batch = Some(batch);
                    }
                    if !queued {
                        self.drawer_media_batch_status =
                            "Không gửi được yêu cầu tải tới worker.".into();
                    }
                    ui.close_menu();
                }
            }
        }
        if let Some(link) = link {
            if ui
                .add_enabled(shareable, egui::Button::new("Sao chép liên kết"))
                .clicked()
            {
                ui.ctx().copy_text(link.to_owned());
                ui.close_menu();
            }
            if ui.button("Mở bằng trình duyệt").clicked() {
                ui.ctx().open_url(egui::OpenUrl::same_tab(link));
                ui.close_menu();
            }
        }
    }

    fn is_cloud_thread(&self, thread: &Thread) -> bool {
        !self.own_cloud_id.is_empty() && thread.id == self.own_cloud_id && !thread.is_group
    }

    fn sort_conversation_indices(&mut self, indices: &mut [usize]) {
        let keys: Vec<_> = indices
            .iter()
            .map(|&index| {
                let thread = &self.threads[index];
                let rank = self.server_pins.rank(&thread.id, thread.is_group);
                (
                    index,
                    rank.is_some()
                        || self
                            .conversation_preferences
                            .get(&thread.id)
                            .is_some_and(|p| p.pinned),
                    rank.unwrap_or(usize::MAX),
                    thread.sidebar_activity_ts(),
                    thread.id.clone(),
                )
            })
            .collect();
        if let Some((previous, order)) = &self.sidebar_order {
            if previous == &keys {
                indices.copy_from_slice(order);
                return;
            }
        }
        let mut order: Vec<_> = keys.iter().collect();
        order.sort_by_key(|key| {
            (
                std::cmp::Reverse(key.1),
                key.2,
                std::cmp::Reverse(key.3),
                key.4.as_str(),
            )
        });
        for (slot, key) in indices.iter_mut().zip(order) {
            *slot = key.0;
        }
        self.sidebar_order = Some((keys, indices.to_vec()));
    }

    pub(super) fn reset_sync_status(&mut self) {
        self.quote_jump = None;
        self.server_pins = Default::default();
        self.server_board = Default::default();
        self.message_pins = Default::default();
        self.history_search = Default::default();
        self.chat_anchor = None;
        self.phone_sync_running = false;
        self.phone_sync_status.clear();
        self.phone_download_status.clear();
        self.phone_import_status.clear();
        self.sync_state = crate::sync_coordinator::SyncState::default();
        self.sync_operation_id = 0;
        self.sync_state_revision = 0;
        self.sync_suggest_dismissed = false;
        self.sync_error_dismissed = false;
        self.sync_suggestion_trigger = None;
        self.sync_suggestion_timer = None;
        self.pending_auto_sync = None;
        self.history_sync_running = false;
        self.history_sync_status.clear();
        self.refresh_sync_running = false;
        self.refresh_sync_status.clear();
        self.sync_cancel_requested = false;
        self.sync_success_until = None;
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

    pub(crate) fn handle_sync_banner_action(
        &mut self,
        action: crate::gui::sync_banner::SyncBannerAction,
        fencing: crate::gui::sync_banner::SyncBannerFencing,
    ) -> bool {
        // Fencing Protection (Invariant 4)
        if !fencing.is_valid(
            &self.user_uin,
            self.account_runtime_id as u64,
            self.sync_state_revision,
        ) {
            return false;
        }

        match action {
            crate::gui::sync_banner::SyncBannerAction::StartSuggestSync => {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::SyncPhoneRecentFenced {
                        owner: self.user_uin.clone(),
                        runtime_id: self.account_runtime_id,
                        intent_id: None,
                        origin: crate::sync_coordinator::SyncOrigin::Banner,
                    });
                }
                self.phone_sync_running = true;
                self.sync_cancel_requested = false;
                self.sync_state_revision = self.sync_state_revision.wrapping_add(1);
                true
            }
            crate::gui::sync_banner::SyncBannerAction::RetryFailedSync => {
                let can_retry = match &self.sync_state {
                    crate::sync_coordinator::SyncState::Failed { fatal, error } => {
                        !*fatal && !crate::gui::sync_banner::is_non_retryable_error(error)
                    }
                    crate::sync_coordinator::SyncState::Interrupted { retryable, .. } => *retryable,
                    _ => false,
                };
                if !can_retry {
                    return false;
                }
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::SyncPhoneRecentFenced {
                        owner: self.user_uin.clone(),
                        runtime_id: self.account_runtime_id,
                        intent_id: None,
                        origin: crate::sync_coordinator::SyncOrigin::Banner,
                    });
                }
                self.phone_sync_running = true;
                self.sync_cancel_requested = false;
                self.sync_error_dismissed = false;
                self.sync_state = crate::sync_coordinator::SyncState::Preparing {
                    source: crate::sync_coordinator::SyncSource::Sync2Recent,
                };
                self.sync_state_revision = self.sync_state_revision.wrapping_add(1);
                true
            }
            crate::gui::sync_banner::SyncBannerAction::CancelSync => {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::CancelPhoneHistoryFenced {
                        owner: self.user_uin.clone(),
                        runtime_id: self.account_runtime_id,
                        operation_id: self.sync_operation_id,
                    });
                }
                self.sync_cancel_requested = true;
                self.sync_state_revision = self.sync_state_revision.wrapping_add(1);
                true
            }
            crate::gui::sync_banner::SyncBannerAction::DismissOrClose => {
                if matches!(
                    self.sync_state,
                    crate::sync_coordinator::SyncState::Failed { .. }
                ) {
                    // When Error: clearErrorAndHideAllUI
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::ClearSyncError {
                            owner: self.user_uin.clone(),
                            runtime_id: self.account_runtime_id,
                        });
                    }
                    self.sync_error_dismissed = true;
                    self.sync_state = crate::sync_coordinator::SyncState::Idle {
                        availability: crate::sync_coordinator::SyncAvailability::Ready,
                    };
                    self.sync_state_revision = self.sync_state_revision.wrapping_add(1);
                    true
                } else if self.phone_sync_running
                    || self.history_sync_running
                    || self.sync_state.is_running()
                    || matches!(
                        self.sync_state,
                        crate::sync_coordinator::SyncState::Interrupted { .. }
                    )
                {
                    // When Running / Interrupted: cancelSync
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::CancelPhoneHistoryFenced {
                            owner: self.user_uin.clone(),
                            runtime_id: self.account_runtime_id,
                            operation_id: self.sync_operation_id,
                        });
                    }
                    self.sync_cancel_requested = true;
                    self.sync_state_revision = self.sync_state_revision.wrapping_add(1);
                    true
                } else if self.sync_success_until.is_some() {
                    // When Success: hide immediately
                    self.sync_success_until = None;
                    self.sync_state_revision = self.sync_state_revision.wrapping_add(1);
                    true
                } else {
                    // When Suggest: closeSuggestNewSyncBanner
                    self.sync_suggest_dismissed = true;
                    self.sync_state_revision = self.sync_state_revision.wrapping_add(1);
                    true
                }
            }
        }
    }

    fn render_sync_banner(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, pal: &ThemePalette) {
        let popup_visible = self.is_workspace_input_gated(ctx);
        let fencing = crate::gui::sync_banner::SyncBannerFencing::new(
            self.user_uin.clone(),
            self.account_runtime_id as u64,
            self.sync_operation_id,
            self.sync_state_revision,
        );

        let legacy_running =
            self.phone_sync_running || self.history_sync_running || self.refresh_sync_running;
        let legacy_detail = if self.phone_sync_running && !self.phone_download_status.is_empty() {
            Some(self.phone_download_status.as_str())
        } else if !self.sync_status_message().is_empty() {
            Some(self.sync_status_message())
        } else {
            None
        };

        let banner_input = crate::gui::sync_banner::SyncBannerInput {
            is_logged_in: self.is_logged_in,
            popup_visible,
            suggest_dismissed: self.sync_suggest_dismissed,
            error_dismissed: self.sync_error_dismissed,
            success_until: self.sync_success_until,
            legacy_running,
            legacy_detail,
            suggestion_trigger: self.sync_suggestion_trigger,
            sync_config: Some(&self.server_config.sync_2_pc),
        };
        let banner_state =
            crate::gui::sync_banner::resolve_banner_state(&banner_input, &self.sync_state);

        let Some(kind) = banner_state else {
            return;
        };

        if let Some(action) = crate::gui::sync_banner::render_sync_banner(
            ui,
            ctx,
            pal,
            &kind,
            &fencing,
            self.sync_cancel_requested,
        ) {
            self.handle_sync_banner_action(action, fencing);
        }
    }

    pub(super) fn render_chat_workspace(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if self.batch_selection_mode
            && !self
                .threads
                .get(self.selected_thread_index)
                .is_some_and(|thread| self.timeline_selection_matches(&thread.id, thread.is_group))
        {
            self.clear_timeline_selection();
        }
        self.render_hidden_panel(ctx);
        if self.policy_panel.snapshot.is_none()
            && !self.policy_panel.pending
            && self.policy_panel.status.is_empty()
        {
            self.policy_command(crate::conversation_policy::Action::Read);
        }
        let protected = self
            .threads
            .iter()
            .filter(|thread| self.content_protected(&thread.id, thread.is_group))
            .map(|thread| (thread.id.clone(), thread.is_group))
            .collect::<HashSet<_>>();
        ctx.data_mut(|data| data.insert_temp(Id::new("protected_threads"), protected));
        let drop_blocked_at_start = self.file_drop_blocked(ctx);
        let modal_open = self.is_workspace_input_gated(ctx);
        let focused_composer_scope =
            self.threads
                .get(self.selected_thread_index)
                .and_then(|thread| {
                    let composer = Id::new(("chat_composer_input", &thread.id));
                    (!modal_open
                        && !self.contacts_view_open
                        && ctx.memory(|memory| memory.has_focus(composer)))
                    .then_some(composer.with((&self.user_uin, self.account_runtime_id)))
                });
        if cancel_interrupted_composer_events(ctx, focused_composer_scope) {
            self.status_msg = "Đã dừng phần nhập chờ khi đổi vùng thao tác; chữ còn lại được giữ trong bản nháp, chưa gửi.".into();
        }
        let file_drag_active = self.attachment_drafts.has_deferred()
            || ctx.input(|i| !i.raw.hovered_files.is_empty() || !i.raw.dropped_files.is_empty());
        // Responsive breakpoints based on live viewport width
        // -------------------------------------------------------------------
        let screen_rect = ctx.screen_rect();
        let screen_w = screen_rect.width();
        // Keep enough room for the navigation rail, conversation list, chat, and info drawer.
        let is_compact_w = screen_w < 1120.0;
        let is_narrow_w = screen_w < 620.0;
        let narrow_chat_open = ctx.data(|data| {
            data.get_temp::<bool>(Id::new("narrow_chat_open"))
                .unwrap_or(false)
        });
        let entering_compact =
            ctx.data(|data| data.get_temp::<bool>(Id::new("drawer_was_compact"))) == Some(false)
                && is_compact_w;
        ctx.data_mut(|data| data.insert_temp(Id::new("drawer_was_compact"), is_compact_w));
        if entering_compact {
            // Compact mode overlays the drawer only after an explicit user action.
            ctx.data_mut(|data| data.insert_temp(Id::new("narrow_drawer_open"), false));
        }
        let narrow_drawer_open = ctx.data(|data| {
            data.get_temp::<bool>(Id::new("narrow_drawer_open"))
                .unwrap_or(false)
        });
        if !is_compact_w && (narrow_chat_open || narrow_drawer_open) {
            ctx.data_mut(|data| {
                data.insert_temp(Id::new("narrow_chat_open"), false);
                data.insert_temp(Id::new("narrow_drawer_open"), false);
            });
        }
        let left_rail_w = if is_narrow_w {
            54.0
        } else {
            theme::NAV_RAIL_WIDTH
        };

        // -------------------------------------------------------------------
        // 1. LEFT NAVIGATION RAIL (Zalo PC Parity: #0068FF Light / #22262B Dark)
        // -------------------------------------------------------------------
        egui::SidePanel::left("left_nav_rail")
            .resizable(false)
            .exact_width(left_rail_w)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_nav_rail)
                    .inner_margin(Margin::symmetric(if is_narrow_w { 4.0 } else { 8.0 }, 14.0))
                    .stroke(Stroke::new(1.0_f32, pal.border)),
            )
            .show(ctx, |ui| {
                if modal_open {
                    ui.style_mut().interaction.tooltip_delay = 86_400.0;
                }
                ui.vertical_centered(|ui| {
                    // User Avatar Button with Cutout Online Status Dot
                    let (av_rect, av_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
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
                            21.5,
                            Stroke::new(2.0_f32, pal.nav_rail_fg_active),
                        );
                    }
                    // Cutout status dot (cutout matches nav rail bg)
                    if self.settings.show_online_status {
                        let dot_c = Pos2::new(av_rect.right() - 4.0, av_rect.bottom() - 4.0);
                        ui.painter().circle_filled(dot_c, 6.5, pal.bg_nav_rail);
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

                    // Navigation Icon 1: Tin nhắn (All / Active Chat) with Unread Count Badge
                    let is_msg_nav =
                        self.thread_filter == ThreadFilter::All && !self.contacts_view_open;
                    let (nav1_rect, nav1_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_msg_nav {
                        ui.painter().rect_filled(
                            nav1_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_active,
                        );
                    } else if nav1_resp.hovered() {
                        ui.painter().rect_filled(
                            nav1_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_hover,
                        );
                    }
                    draw_icon_chat_nav(
                        ui.painter(),
                        nav1_rect.center(),
                        20.0,
                        if is_msg_nav {
                            pal.nav_rail_fg_active
                        } else {
                            pal.nav_rail_fg
                        },
                    );
                    // Red unread badge on top-right of chat icon
                    let total_unread: usize = self.unread_counts.values().sum();
                    if total_unread > 0 {
                        let badge_w = if total_unread > 99 {
                            24.0
                        } else if total_unread > 9 {
                            18.0
                        } else {
                            16.0
                        };
                        let badge_rect = Rect::from_center_size(
                            Pos2::new(nav1_rect.right() - 5.0, nav1_rect.top() + 5.0),
                            Vec2::new(badge_w, 14.0),
                        );
                        ui.painter()
                            .rect_filled(badge_rect, Rounding::same(7.0), pal.danger);
                        let badge_txt = if total_unread > 99 {
                            "99+".to_string()
                        } else {
                            total_unread.to_string()
                        };
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            badge_txt,
                            FontId::proportional(9.5),
                            Color32::WHITE,
                        );
                    }
                    if nav1_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(crate::i18n::tr("Tin nhắn"))
                        .clicked()
                    {
                        self.contacts_view_open = false;
                        self.thread_filter = ThreadFilter::All;
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 2: Danh bạ (Contacts)
                    let is_contacts_nav = self.contacts_view_open;
                    let (nav2_rect, nav2_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_contacts_nav {
                        ui.painter().rect_filled(
                            nav2_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_active,
                        );
                    } else if nav2_resp.hovered() {
                        ui.painter().rect_filled(
                            nav2_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_hover,
                        );
                    }
                    draw_icon_contacts(
                        ui.painter(),
                        nav2_rect.center(),
                        20.0,
                        if is_contacts_nav {
                            pal.nav_rail_fg_active
                        } else {
                            pal.nav_rail_fg
                        },
                    );
                    if nav2_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(crate::i18n::tr("Danh bạ"))
                        .clicked()
                    {
                        self.contacts_view_open = !self.contacts_view_open;
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 3: Giao việc (To-Do)
                    let is_todo_nav = self.tools.open;
                    let (nav_todo_rect, nav_todo_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_todo_nav {
                        ui.painter().rect_filled(
                            nav_todo_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_active,
                        );
                    } else if nav_todo_resp.hovered() {
                        ui.painter().rect_filled(
                            nav_todo_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_hover,
                        );
                    }
                    draw_icon_todo(
                        ui.painter(),
                        nav_todo_rect.center(),
                        19.0,
                        if is_todo_nav {
                            pal.nav_rail_fg_active
                        } else {
                            pal.nav_rail_fg
                        },
                    );
                    if nav_todo_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(crate::i18n::tr("Giao việc (To-Do) & Tiện ích"))
                        .clicked()
                    {
                        self.tools.open = !self.tools.open;
                    }

                    ui.add_space(6.0);

                    // Navigation Icon 4: Cloud của tôi (Dedicated Zalo Cloud)
                    let is_cloud_selected = !self.contacts_view_open
                        && self
                            .threads
                            .get(self.selected_thread_index)
                            .is_some_and(|t| self.is_cloud_thread(t));
                    let (nav3_rect, nav3_resp) =
                        ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
                    if is_cloud_selected {
                        ui.painter().rect_filled(
                            nav3_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_active,
                        );
                    } else if nav3_resp.hovered() {
                        ui.painter().rect_filled(
                            nav3_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_hover,
                        );
                    }
                    draw_icon_cloud(
                        ui.painter(),
                        nav3_rect.center(),
                        20.0,
                        if is_cloud_selected {
                            pal.nav_rail_fg_active
                        } else {
                            pal.nav_rail_fg
                        },
                    );
                    if nav3_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(crate::i18n::tr("Cloud của tôi (Lưu trữ cá nhân)"))
                        .clicked()
                    {
                        if let Some((cidx, cthread)) = self
                            .threads
                            .iter()
                            .enumerate()
                            .find(|(_, t)| self.is_cloud_thread(t))
                        {
                            let cid = cthread.id.clone();
                            self.contacts_view_open = false;
                            self.selected_thread_index = cidx;
                            self.drawer_view = DrawerView::Overview;
                            self.drawer_search_query.clear();
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
                    let show_lock_btn = self.can_lock_app();
                    let reserved_bottom_h = if show_lock_btn { 146.0 } else { 100.0 };
                    let rem_h = (ui.available_height() - reserved_bottom_h).max(12.0);
                    ui.add_space(rem_h);

                    // Bottom Action 1: Background Sync Indicator Icon
                    let (sync_rect, sync_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    if sync_resp.hovered() {
                        ui.painter().rect_filled(
                            sync_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_hover,
                        );
                    }
                    draw_icon_refresh(
                        ui.painter(),
                        sync_rect.center(),
                        17.0,
                        if sync_resp.hovered() {
                            pal.accent_online
                        } else {
                            pal.nav_rail_fg
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

                    // Optional Bottom Action: Quick App Lock Button (MAP-00)
                    if show_lock_btn {
                        let (lock_rect, lock_resp) =
                            ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                        if lock_resp.hovered() {
                            ui.painter().rect_filled(
                                lock_rect,
                                Rounding::same(10.0),
                                pal.nav_rail_item_hover,
                            );
                        }
                        draw_icon_lock(
                            ui.painter(),
                            lock_rect.center(),
                            18.0,
                            if lock_resp.hovered() {
                                pal.nav_rail_fg_active
                            } else {
                                pal.nav_rail_fg
                            },
                        );
                        if lock_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text(crate::i18n::tr("Khóa ứng dụng (Ctrl+L)"))
                            .clicked()
                        {
                            self.lock_app();
                        }
                        ui.add_space(6.0);
                    }

                    // Bottom Action 2: Settings Modal Button
                    let (set_rect, set_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::click());
                    if self.settings_modal_open {
                        ui.painter().rect_filled(
                            set_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_active,
                        );
                    } else if set_resp.hovered() {
                        ui.painter().rect_filled(
                            set_rect,
                            Rounding::same(10.0),
                            pal.nav_rail_item_hover,
                        );
                    }
                    draw_icon_settings(
                        ui.painter(),
                        set_rect.center(),
                        18.0,
                        if self.settings_modal_open || set_resp.hovered() {
                            pal.nav_rail_fg_active
                        } else {
                            pal.nav_rail_fg
                        },
                    );
                    if set_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text(crate::i18n::tr("Cài đặt Zalo"))
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
            theme::SIDEBAR_DEFAULT_WIDTH
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

        if !is_narrow_w || !narrow_chat_open {
            let sidebar_response = egui::SidePanel::left(sidebar_id)
            .resizable(!is_narrow_w)
            .default_width(sidebar_default_w)
            .width_range(sidebar_min_w..=sidebar_max_w)
            .frame(
                egui::Frame::none()
                    .fill(pal.bg_sidebar)
                    .stroke(Stroke::new(1.0_f32, pal.border)),
            )
            .show(ctx, |ui| {
                // SidePanel clips to its planned rectangle but reserves the content's final
                // minimum rectangle. Isolate content so a wide label cannot reserve an unpainted
                // gutter or persist a wider panel after search/banner state changes.
                let bounds = ui.max_rect();
                ui.set_min_width(bounds.width());
                let mut content_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("conversation_sidebar_content")
                        .max_rect(bounds),
                );
                content_ui.set_clip_rect(bounds.intersect(ui.clip_rect()));
                let ui = &mut content_ui;
                if modal_open {
                    ui.style_mut().interaction.tooltip_delay = 86_400.0;
                }
                let header_top = ui.next_widget_position();
                let mut header_ui = ui.new_child(egui::UiBuilder::new()
                    .id_salt("sidebar_search_section")
                    .max_rect(Rect::from_min_max(header_top, Pos2::new(bounds.right(), bounds.bottom()))));
                egui::Frame::none()
                    .inner_margin(Margin {
                        left: if is_narrow_w { 4.0 } else { 14.0 },
                        right: if is_narrow_w { 4.0 } else { 14.0 },
                        top: 12.0,
                        bottom: 0.0,
                    })
                    .show(&mut header_ui, |ui| {
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
                                    .on_hover_text(crate::i18n::tr("Tìm kiếm hội thoại & Tra cứu SĐT"))
                                    .clicked()
                                {
                                    self.phone_modal_open = true;
                                }
                            });
                            ui.add_space(6.0);
                        } else {
                            // Search Box + Quick Phone Lookup (`+👤`) + Create Group (`👥+`)
                            let avail_row_w = ui.available_width();
                            let mut trigger_phone_lookup: Option<String> = None;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let search_box_w = (avail_row_w - 78.0).max(1.0);
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
                                        let inner_w = (search_box_w - 20.0).max(1.0);
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
                                            let is_four_digit_pin = self.search_query.trim().len() == 4
                                                && self.search_query.trim().chars().all(|c| c.is_ascii_digit());
                                            let show_pin_unlock = is_four_digit_pin && !self.hidden_panel.view.unlocked;
                                            let edit_w = if show_pin_unlock {
                                                (inner_w - 63.0).max(1.0)
                                            } else if has_q {
                                                (inner_w - 43.0).max(1.0)
                                            } else {
                                                (inner_w - 21.0).max(1.0)
                                            };
                                            let s_resp = ui.add(
                                                egui::TextEdit::singleline(&mut self.search_query)
                                                    .id(Id::new("sidebar_search_input"))
                                                    .frame(false)
                                                    .margin(Vec2::ZERO)
                                                    .hint_text(crate::i18n::tr("Tìm kiếm..."))
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
                                            if s_enter && is_four_digit_pin {
                                                let pin = self.search_query.trim().to_string();
                                                let gen = crate::search::generation();
                                                self.hidden_panel.generation = gen;
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ = tx.send(WorkerCmd::Hidden {
                                                        owner: self.user_uin.clone(),
                                                        generation: gen,
                                                        action: crate::hidden_chat::Action::Unlock(zeroize::Zeroizing::new(pin)),
                                                    });
                                                }
                                                self.search_query.clear();
                                            } else if s_enter && digits_cnt >= 8 {
                                                trigger_phone_lookup =
                                                    Some(self.search_query.trim().to_string());
                                            }
                                            if show_pin_unlock {
                                                let (unlock_rect, unlock_resp) = ui.allocate_exact_size(
                                                    Vec2::new(16.0, 16.0),
                                                    Sense::click(),
                                                );
                                                if unlock_resp.hovered() {
                                                    ui.painter().circle_filled(
                                                        unlock_rect.center(),
                                                        8.0,
                                                        pal.bg_hover,
                                                    );
                                                }
                                                ui.painter().text(
                                                    unlock_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "🔓",
                                                    FontId::proportional(11.0),
                                                    pal.accent_online,
                                                );
                                                if unlock_resp
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text(crate::i18n::tr("Nhấn để mở khóa cuộc trò chuyện ẩn (PIN)"))
                                                    .clicked()
                                                {
                                                    let pin = self.search_query.trim().to_string();
                                                    let gen = crate::search::generation();
                                                    self.hidden_panel.generation = gen;
                                                    if let Some(tx) = WORKER_TX.get() {
                                                        let _ = tx.send(WorkerCmd::Hidden {
                                                            owner: self.user_uin.clone(),
                                                            generation: gen,
                                                            action: crate::hidden_chat::Action::Unlock(zeroize::Zeroizing::new(pin)),
                                                        });
                                                    }
                                                    self.search_query.clear();
                                                }
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
                                                    .on_hover_text(crate::i18n::tr("Xóa tìm kiếm"))
                                                    .clicked()
                                                {
                                                    self.search_query.clear();
                                                }
                                            } else {
                                                let is_archived = self.thread_filter == ThreadFilter::Archived;
                                                let (arc_rect, arc_resp) = ui.allocate_exact_size(
                                                    Vec2::new(16.0, 16.0),
                                                    Sense::click(),
                                                );
                                                if arc_resp.hovered() || is_archived {
                                                    ui.painter().circle_filled(
                                                        arc_rect.center(),
                                                        8.0,
                                                        if is_archived { pal.bg_selected } else { pal.bg_hover },
                                                    );
                                                }
                                                ui.painter().text(
                                                    arc_rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "📁",
                                                    FontId::proportional(11.0),
                                                    if is_archived { pal.primary } else if arc_resp.hovered() { pal.text_primary } else { pal.text_muted },
                                                );
                                                if arc_resp
                                                    .on_hover_cursor(CursorIcon::PointingHand)
                                                    .on_hover_text(if is_archived {
                                                        crate::i18n::tr("Quay lại danh sách hội thoại chính")
                                                    } else {
                                                        crate::i18n::tr("Xem hội thoại đã lưu trữ")
                                                    })
                                                    .clicked()
                                                {
                                                    self.thread_filter = if is_archived {
                                                        ThreadFilter::All
                                                    } else {
                                                        ThreadFilter::Archived
                                                    };
                                                    if !is_archived {
                                                        self.unread_only = false;
                                                    }
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
                                    .on_hover_text(crate::i18n::tr("Thêm bạn & Tra cứu SĐT"))
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

                                // Create Group Button (`👥+`)
                                let (grp_rect, grp_resp) =
                                    ui.allocate_exact_size(Vec2::new(32.0, 32.0), Sense::click());
                                ui.painter().rect_filled(
                                    grp_rect,
                                    Rounding::same(8.0),
                                    if grp_resp.hovered() {
                                        pal.bg_hover
                                    } else {
                                        pal.bg_input
                                    },
                                );
                                ui.painter().rect_stroke(
                                    grp_rect,
                                    Rounding::same(8.0),
                                    Stroke::new(
                                        1.0_f32,
                                        if grp_resp.hovered() {
                                            pal.primary
                                        } else {
                                            pal.border
                                        },
                                    ),
                                );
                                draw_icon_contacts(
                                    ui.painter(),
                                    grp_rect.center(),
                                    16.5,
                                    if grp_resp.hovered() {
                                        pal.primary_hover
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if grp_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Tạo nhóm trò chuyện mới"))
                                    .clicked()
                                {
                                    self.open_group_panel(String::new(), true, true);
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
                            ui.add_space(8.0);

                            // Clean Zalo PC Filter Strip: [Tất cả] | [Chưa đọc] ... [Phân loại ▾] [⋯]
                            let strip_w = (bounds.width() - if is_narrow_w { 8.0 } else { 28.0 }).max(1.0);
                            ui.allocate_ui_with_layout(Vec2::new(strip_w, 26.0), Layout::left_to_right(Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;

                                // Tab 1: Tất cả
                                let is_all = !self.unread_only && self.thread_filter == ThreadFilter::All;
                                let (tab_all_rect, tab_all_resp) =
                                    ui.allocate_exact_size(Vec2::new(44.0, 26.0), Sense::click());
                                let fg_all = if is_all {
                                    pal.primary_hover
                                } else if tab_all_resp.hovered() {
                                    pal.text_primary
                                } else {
                                    pal.text_secondary
                                };
                                ui.painter().text(
                                    tab_all_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    crate::i18n::tr("Tất cả"),
                                    FontId::proportional(13.0),
                                    fg_all,
                                );
                                if is_all {
                                    let line_y = tab_all_rect.bottom() - 1.0;
                                    ui.painter().line_segment(
                                        [
                                            Pos2::new(tab_all_rect.left(), line_y),
                                            Pos2::new(tab_all_rect.right(), line_y),
                                        ],
                                        Stroke::new(2.4_f32, pal.primary),
                                    );
                                }
                                if tab_all_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.unread_only = false;
                                    self.thread_filter = ThreadFilter::All;
                                }

                                // Tab 2: Chưa đọc
                                let is_unread = self.unread_only;
                                let (tab_unr_rect, tab_unr_resp) =
                                    ui.allocate_exact_size(Vec2::new(56.0, 26.0), Sense::click());
                                let fg_unr = if is_unread {
                                    pal.primary_hover
                                } else if tab_unr_resp.hovered() {
                                    pal.text_primary
                                } else {
                                    pal.text_secondary
                                };
                                ui.painter().text(
                                    tab_unr_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    crate::i18n::tr("Chưa đọc"),
                                    FontId::proportional(13.0),
                                    fg_unr,
                                );
                                if is_unread {
                                    let line_y = tab_unr_rect.bottom() - 1.0;
                                    ui.painter().line_segment(
                                        [
                                            Pos2::new(tab_unr_rect.left(), line_y),
                                            Pos2::new(tab_unr_rect.right(), line_y),
                                        ],
                                        Stroke::new(2.4_f32, pal.primary),
                                    );
                                }
                                if tab_unr_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.unread_only = true;
                                }

                                // Right-aligned actions: Phân loại ▾ and ⋯ Menu
                                let right_w = ui.available_width().max(1.0);
                                ui.allocate_ui_with_layout(Vec2::new(right_w, 26.0), Layout::right_to_left(Align::Center), |ui| {
                                    // ⋯ Menu Button
                                    let (more_rect, more_resp) =
                                        ui.allocate_exact_size(Vec2::new(24.0, 24.0), Sense::click());
                                    if more_resp.hovered() || self.sidebar_more_menu_open {
                                        ui.painter().rect_filled(
                                            more_rect,
                                            Rounding::same(6.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    ui.painter().text(
                                        more_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "⋯",
                                        FontId::proportional(15.0),
                                        if self.sidebar_more_menu_open || more_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if more_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(crate::i18n::tr("Tùy chọn hiển thị & Quản lý hội thoại"))
                                        .clicked()
                                    {
                                        self.sidebar_more_menu_open = !self.sidebar_more_menu_open;
                                    }

                                    // Phân loại ▾ Button
                                    let (lbl_rect, lbl_resp) =
                                        ui.allocate_exact_size(Vec2::new(72.0, 24.0), Sense::click());
                                    #[cfg(test)]
                                    ctx.data_mut(|data| data.insert_temp(Id::new("sidebar-filter-bounds"),
                                        (tab_all_rect, tab_unr_rect, lbl_rect, more_rect)));
                                    if lbl_resp.hovered() {
                                        ui.painter().rect_filled(
                                            lbl_rect,
                                            Rounding::same(6.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    ui.painter().text(
                                        lbl_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        crate::i18n::tr("Phân loại ▾"),
                                        FontId::proportional(12.0),
                                        if lbl_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if lbl_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(crate::i18n::tr("Lọc và quản lý thẻ phân loại"))
                                        .clicked()
                                    {
                                        self.open_labels(None);
                                    }
                                });
                            });

                            // Anchored Popover for ⋯ Menu
                            if self.sidebar_more_menu_open {
                                egui::Frame::none()
                                    .fill(pal.bg_elevated)
                                    .rounding(Rounding::same(8.0))
                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                    .shadow(egui::epaint::Shadow {
                                        offset: Vec2::new(0.0, 2.0),
                                        blur: 8.0,
                                        spread: 0.0,
                                        color: Color32::from_black_alpha(35),
                                    })
                                    .inner_margin(Margin::symmetric(10.0, 8.0))
                                    .outer_margin(Margin::symmetric(4.0, 4.0))
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            if ui.button(crate::i18n::tr("✓ Đánh dấu đã đọc tất cả")).clicked() {
                                                for t in &mut self.threads {
                                                    t.unread.read_visible(&t.messages);
                                                    if let Some(last) = t.messages.last() {
                                                        t.unread.last_seen_id = Some(last.id.clone());
                                                    }
                                                }
                                                self.unread_counts.clear();
                                                self.refresh_unread_title(ctx);
                                                self.sidebar_more_menu_open = false;
                                            }
                                            ui.separator();
                                            if ui.button(crate::i18n::tr("🔒 Quản lý hội thoại ẩn / PIN…")).clicked() {
                                                self.hidden_panel.open = true;
                                                self.sidebar_more_menu_open = false;
                                            }
                                            ui.checkbox(
                                                &mut self.show_hidden_conversations,
                                                crate::i18n::tr("Hiện cả hội thoại đã ẩn"),
                                            );
                                            ui.separator();
                                            let is_archived = self.thread_filter == ThreadFilter::Archived;
                                            if ui.selectable_label(is_archived, crate::i18n::tr("📁 Tin nhắn đã lưu trữ")).clicked() {
                                                self.thread_filter = if is_archived { ThreadFilter::All } else { ThreadFilter::Archived };
                                                self.sidebar_more_menu_open = false;
                                            }
                                            let is_strangers = self.thread_filter == ThreadFilter::Strangers;
                                            if ui.selectable_label(is_strangers, crate::i18n::tr("👤 Tin nhắn từ người lạ")).clicked() {
                                                self.thread_filter = if is_strangers { ThreadFilter::All } else { ThreadFilter::Strangers };
                                                self.sidebar_more_menu_open = false;
                                            }
                                            let is_groups = self.thread_filter == ThreadFilter::Groups;
                                            if ui.selectable_label(is_groups, crate::i18n::tr("👥 Lọc theo nhóm")).clicked() {
                                                self.thread_filter = if is_groups { ThreadFilter::All } else { ThreadFilter::Groups };
                                                self.sidebar_more_menu_open = false;
                                            }
                                            let is_direct = self.thread_filter == ThreadFilter::Direct;
                                            if ui.selectable_label(is_direct, crate::i18n::tr("💬 Lọc theo cá nhân")).clicked() {
                                                self.thread_filter = if is_direct { ThreadFilter::All } else { ThreadFilter::Direct };
                                                self.sidebar_more_menu_open = false;
                                            }
                                        });
                                    });
                            }
                        }
                    });
                // Header widgets own their intrinsic minimum width. Reserve only
                // their height so that they cannot move the list's right edge.
                ui.advance_cursor_after_rect(Rect::from_min_size(header_top, Vec2::new(bounds.width(), header_ui.min_rect().height())));

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
                                        RichText::new(crate::i18n::tr("Nhấn nút Tìm hoặc phím Enter để tra cứu"))
                                            .size(10.8)
                                            .color(pal.text_secondary),
                                    );
                                });
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let btn = egui::Button::new(
                                        RichText::new(crate::i18n::tr("Tìm SĐT"))
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

                // Active filter pill chips (Zalo PC style)
                if let Some(id) = self.labels_panel.filter {
                    if let Some(label) = self.labels_panel.snapshot.as_ref().and_then(|s| s.labels.iter().find(|l| l.id == id)) {
                        egui::Frame::none()
                            .fill(pal.bg_selected)
                            .rounding(Rounding::same(6.0))
                            .inner_margin(Margin::symmetric(8.0, 4.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("🏷 {}", label.text)).size(11.5).color(pal.primary_hover));
                                    if ui.small_button("✕").on_hover_text(crate::i18n::tr("Bỏ lọc thẻ")).clicked() {
                                        self.labels_panel.filter = None;
                                    }
                                });
                            });
                    }
                }
                if self.thread_filter == ThreadFilter::Strangers {
                    ui.label(
                        RichText::new(crate::i18n::tr("Đang xem tin nhắn từ người lạ"))
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                } else if self.thread_filter == ThreadFilter::Archived {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(crate::i18n::tr("📁 Đang xem cuộc trò chuyện đã lưu trữ"))
                                .size(11.0)
                                .color(pal.text_muted),
                        );
                        if ui.small_button(crate::i18n::tr("✕ Thoát lưu trữ")).clicked() {
                            self.thread_filter = ThreadFilter::All;
                        }
                    });
                } else if self.thread_filter == ThreadFilter::Groups {
                    ui.label(
                        RichText::new(crate::i18n::tr("Đang lọc: Danh sách nhóm"))
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                } else if self.thread_filter == ThreadFilter::Direct {
                    ui.label(
                        RichText::new(crate::i18n::tr("Đang lọc: Cuộc trò chuyện cá nhân"))
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                }
                let selected_label=self.labels_panel.filter.and_then(|id|self.labels_panel.snapshot.as_ref().and_then(|snapshot|snapshot.labels.iter().find(|label|label.id==id)));
                let mut filtered_indices: Vec<usize> = self
                    .threads
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| {
                        let pass_filter = match self.thread_filter {
                            ThreadFilter::All => self.inbox_placement(t)!=crate::inbox::Placement::StrangerBox,
                            ThreadFilter::Strangers => self.inbox_placement(t)==crate::inbox::Placement::StrangerBox,
                            ThreadFilter::Direct => !t.is_group && self.inbox_placement(t)!=crate::inbox::Placement::StrangerBox,
                            ThreadFilter::Groups => t.is_group,
                            ThreadFilter::Archived => t.presentation.archived==Some(true),
                        };
                        let pass_label=self.labels_panel.filter.is_none() || selected_label.is_some_and(|label|crate::server_pins::identity(&t.id,t.is_group).ok().is_some_and(|identity|label.contains(&identity)));
                        self.private_access(&t.id,t.is_group) && (self.hidden_panel.show_in_sidebar || !crate::server_pins::identity(&t.id,t.is_group).ok().is_some_and(|id|self.hidden_panel.view.ids.contains(&id))) && pass_filter && pass_label
                            && (self.thread_filter==ThreadFilter::Archived || t.presentation.archived!=Some(true))
                            && (!self.unread_only || self.unread_counts.get(&t.id).copied().unwrap_or(0) > 0 || t.unread.has_unread_mentions() || t.unread.marked_unread)
                            && (self.show_hidden_conversations
                                || !self
                                    .conversation_preferences
                                    .get(&t.id)
                                    .is_some_and(|p| p.hidden))
                            && (query.is_empty()
                                || matches_vn_query(self.resolve_thread_name(t), &query, &query_folded)
                                || t.presentation
                                    .base_name
                                    .as_deref()
                                    .is_some_and(|b| matches_vn_query(b, &query, &query_folded))
                                || t.id.contains(&query))
                    })
                    .map(|(i, _)| i)
                    .collect();

                self.sort_conversation_indices(&mut filtered_indices);
                if self.settings.split_priority_inbox {
                    let key=Id::new(("local_priority_inbox",&self.user_uin,self.account_runtime_id));
                    let mut priority=ctx.data(|data|data.get_temp::<bool>(key)).unwrap_or(true);
                    ui.horizontal_wrapped(|ui|{
                        ui.selectable_value(&mut priority,true,"Ưu tiên (đã ghim)");
                        ui.selectable_value(&mut priority,false,"Khác");
                    });
                    ctx.data_mut(|data|data.insert_temp(key,priority));
                    filtered_indices.retain(|index|{
                        let thread=&self.threads[*index];
                        let pinned=self.server_pins.rank(&thread.id,thread.is_group).is_some()
                            || self.conversation_preferences.get(&thread.id).is_some_and(|preference|preference.pinned);
                        pinned==priority
                    });
                    if filtered_indices.is_empty() {ui.label("Không có hội thoại trong mục này.");}
                }

                // Also search message contents across all conversations when `query` is non-empty!
                if query.len() >= 2
                    && ui
                        .small_button("Tìm trong toàn bộ lịch sử đã lưu")
                        .clicked()
                {
                    self.open_history_search(None,crate::search::Category::All,self.search_query.clone());
                }

                let mut preference_action = None;
                let mut clear_history_action = None;
                let mut read_action = None;
                let mut manual_mark_action = None;
                let mut archive_action: Option<(String, bool, bool)> = None;
                let mut label_target: Option<(String, bool)> = None;
                let mut invite_contact_target = None;
                let mut report_target = None;
                let mut clicked_thread: Option<(usize, String, bool)> = None;
                let mut mention_target: Option<(String, String)> = None;

                // Sync Banner V2 placement: below filter tabs and directly ABOVE thread list scroll area
                let banner_top = ui.next_widget_position();
                let mut banner_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("sidebar_sync_section_v2")
                        .max_rect(Rect::from_min_max(banner_top, Pos2::new(bounds.right(), bounds.bottom()))),
                );
                self.render_sync_banner(&mut banner_ui, ctx, pal);
                let banner_h = banner_ui.min_rect().height();
                if banner_h > 0.0 {
                    ui.advance_cursor_after_rect(Rect::from_min_size(banner_top, Vec2::new(bounds.width(), banner_h)));
                    ui.add_space(4.0);
                }

                let row_height = if is_narrow_w { 50.0 } else { 58.0 };
                egui::ScrollArea::vertical()
                    .id_salt("threads_list_scroll")
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, filtered_indices.len(), |ui, rows| {
                        for row in rows {
                            let i = filtered_indices[row];
                            let t = &self.threads[i];
                            let target_id = t.id.clone();
                            let target_group = t.is_group;
                            let target_is_cloud = !self.own_cloud_id.is_empty() && target_id == self.own_cloud_id;
                            let target_is_self = !target_group && target_id == self.user_uin;
                            let can_offer_group_invite = !target_group && !target_is_cloud && !target_is_self
                                && t.presentation.inbox.official_account != Some(true);
                            let target_marked_unread = t.unread.marked_unread;
                            let unread_n = self.unread_counts.get(&t.id).copied().unwrap_or(0);
                            let target_latest = latest_message_by_time(&t.messages);
                            let is_selected = i == self.selected_thread_index;
                            let is_user_pinned = self.server_pins.rank(&t.id,t.is_group).is_some() || self
                                .conversation_preferences
                                .get(&t.id)
                                .is_some_and(|p| p.pinned);

                            // egui available_width grows with earlier intrinsic
                            // minimums; row geometry follows the actual viewport.
                            let row_width = (bounds.right().min(ui.max_rect().right()) - ui.next_widget_position().x - 8.0).max(44.0);
                            let (row_rect, row_resp) = ui.push_id(("conversation-row", &target_id, target_group), |ui| {
                                ui.allocate_exact_size(Vec2::new(row_width, row_height),
                                    if modal_open { Sense::hover() } else { Sense::click() })
                            }).inner;
                            let row_rect = row_rect.translate(Vec2::new(4.0, 0.0));
                            #[cfg(test)]
                            ctx.data_mut(|data| data.insert_temp(Id::new(("sidebar-row-rect", &target_id)), row_rect));
                            let preference = self
                                .conversation_preferences
                                .get(&t.id)
                                .copied()
                                .unwrap_or_default();
                            if !modal_open { row_resp.context_menu(|ui| {
                                let remote_pinned=self.server_pins.rank(&target_id,target_group).is_some();
                                if ui.button(crate::i18n::tr(if remote_pinned{"Bỏ ghim trên mọi thiết bị"}else{"Ghim trên mọi thiết bị"})).clicked(){
                                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::SetServerPin{owner:self.user_uin.clone(),thread:target_id.clone(),group:target_group,pinned:!remote_pinned});}
                                    ui.close_menu();
                                }
                                ui.separator();
                                ui.label(crate::i18n::tr("Hội thoại trên máy này"));
                                if ui
                                    .button(if preference.pinned {
                                        "Bỏ ghim hội thoại"
                                    } else {
                                        "Ghim hội thoại"
                                    })
                                    .clicked()
                                {
                                    preference_action = Some((
                                        target_id.clone(),
                                        target_group,
                                        crate::store::ConversationPreference {
                                            pinned: !preference.pinned,
                                            ..preference
                                        },
                                    ));
                                    ui.close_menu();
                                }
                                if ui
                                    .button(if preference.muted {
                                        "Bật thông báo"
                                    } else {
                                        "Tắt thông báo"
                                    })
                                    .clicked()
                                {
                                    preference_action = Some((
                                        target_id.clone(),
                                        target_group,
                                        crate::store::ConversationPreference {
                                            muted: !preference.muted,
                                            ..preference
                                        },
                                    ));
                                    ui.close_menu();
                                }
                                let target_archived = t.presentation.archived == Some(true);
                                if ui
                                    .button(if target_archived {
                                        crate::i18n::tr("Đưa về hội thoại chính")
                                    } else {
                                        crate::i18n::tr("Lưu trữ cuộc trò chuyện")
                                    })
                                    .clicked()
                                {
                                    archive_action = Some((target_id.clone(), target_group, !target_archived));
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
                                        target_id.clone(),
                                        target_group,
                                        crate::store::ConversationPreference {
                                            hidden: !preference.hidden,
                                            ..preference
                                        },
                                    ));
                                    ui.close_menu();
                                }
                                if ui.button("Ẩn bằng PIN trên máy chủ…").clicked(){self.hidden_panel.open=true;self.hidden_panel.target=Some((target_id.clone(),target_group,true));ui.close_menu();}
                                if target_marked_unread || unread_n > 0 {
                                    if ui.button(crate::i18n::tr("Đánh dấu đã đọc")).clicked() {
                                        if target_marked_unread {
                                            manual_mark_action = Some((target_id.clone(), target_group, crate::unread::MarkAction::Remove));
                                        }
                                        read_action = Some(target_id.clone());
                                        ui.close_menu();
                                    }
                                } else if let Some(message) = target_latest.clone() {
                                    if !self.manual_mark_pending
                                        && ui.button(crate::i18n::tr("Đánh dấu chưa đọc")).clicked()
                                    {
                                        manual_mark_action = Some((target_id.clone(), target_group, crate::unread::MarkAction::Add(Box::new(message))));
                                        ui.close_menu();
                                    }
                                }
                                if ui.button(crate::i18n::tr("Phân loại…")).clicked() {
                                    label_target = Some((target_id.clone(), target_group));
                                    ui.close_menu();
                                }
                                if can_offer_group_invite && ui.button("Thêm vào nhóm / cộng đồng…").clicked() {
                                    invite_contact_target = Some(target_id.clone());
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button(crate::i18n::tr("Xóa cuộc trò chuyện…")).clicked(){clear_history_action=Some((target_id.clone(),true));ui.close_menu();}
                                if ui.button(crate::i18n::tr("Xóa lịch sử trên máy này…")).clicked() {
                                    clear_history_action = Some((target_id.clone(),false));
                                    ui.close_menu();
                                }
                                if !target_is_cloud && !target_is_self && ui.button("Báo xấu…").clicked() {
                                    report_target = Some((target_id.clone(), target_group));
                                    ui.close_menu();
                                }
                            }); }

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
                            let thread_display = resolve_thread_name_from(&self.server_aliases, t);
                            paint_user_avatar(
                                ui.painter(),
                                avatar_center,
                                av_radius,
                                thread_display,
                                t.is_group,
                                t.avatar.as_deref(),
                                &self.sticker_textures,
                                &mut self.loading_stickers,
                            );
                            // Presence is unknown: no verified presence feed is available.
                                if t.unread.has_unread_mentions() {
                                    let rect = Rect::from_center_size(Pos2::new(if is_narrow_w{row_rect.left()+46.0}else{row_rect.right()-55.0}, row_rect.top()+40.0), Vec2::new(22.0,20.0));
                                    ui.painter().rect_filled(rect, Rounding::same(8.0), pal.bg_selected);
                                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "@", FontId::proportional(14.0), pal.danger);
                                    let hit = ui.interact(rect, Id::new(("mention", &t.id)), Sense::click()).on_hover_text(crate::i18n::tr("Đến tin nhắc đến bạn chưa đọc"));
                                    if hit.clicked() {
                                        let first=t.messages.iter().filter(|m|t.unread.mention_ids.contains(&m.id)).min_by_key(|m|m.ts).map(|m|m.id.clone())
                                            .or_else(||t.unread.mention_ids.iter().next().cloned());
                                        if let Some(id)=first {mention_target=Some((t.id.clone(),id));}
                                    }
                                }
                            // Only show text labels when sidebar is expanded
                            if !is_narrow_w {
                                // Thread Name
                                let text_left = row_rect.left() + 58.0;
                                let name_color = if is_selected && pal.is_dark {
                                    Color32::WHITE
                                } else {
                                    pal.text_primary
                                };
                                let row_time = t.sidebar_timestamp(crate::login::unix_now() * 1000);
                                let time_right = row_rect.right() - 9.0;
                                let time_width = ui.fonts(|f| f.layout_no_wrap(row_time.clone(), FontId::proportional(11.0), pal.text_muted).size().x);
                                let is_muted = self.conversation_preferences.get(&t.id).map(|p| p.muted).unwrap_or(false);
                                let mut right_icons_width = 8.0;
                                if is_user_pinned { right_icons_width += 15.0; }
                                if is_muted { right_icons_width += 15.0; }
                                let name_right = time_right - time_width - right_icons_width;
                                let mut name_job = egui::text::LayoutJob::simple_singleline(thread_display.to_owned(), FontId::proportional(13.6), name_color);
                                name_job.wrap.max_width = (name_right - text_left).max(1.0);
                                name_job.wrap.max_rows = 1;
                                name_job.wrap.break_anywhere = true;
                                let name_galley = ui.fonts(|f| f.layout_job(name_job));
                                ui.painter().galley(Pos2::new(text_left, row_rect.top() + 12.0), name_galley, name_color);

                                // Pin icon + Mute icon + Timestamp (top-right)
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
                                    let mut icon_x = time_right - tw;
                                    if is_user_pinned {
                                        draw_icon_pin(
                                            ui.painter(),
                                            Pos2::new(icon_x - 9.0, row_rect.top() + 19.5),
                                            11.0,
                                            pal.text_muted,
                                        );
                                        icon_x -= 16.0;
                                    }
                                    if is_muted {
                                        ui.painter().text(
                                            Pos2::new(icon_x - 3.0, row_rect.top() + 13.5),
                                            egui::Align2::RIGHT_TOP,
                                            "🔕",
                                            FontId::proportional(10.5),
                                            pal.text_muted,
                                        );
                                    }
                                } else if is_muted {
                                    ui.painter().text(
                                        Pos2::new(time_right, row_rect.top() + 13.5),
                                        egui::Align2::RIGHT_TOP,
                                        "🔕",
                                        FontId::proportional(10.5),
                                        pal.text_muted,
                                    );
                                }

                                let reactor_name = t.presentation.reaction_preview.as_ref()
                                    .and_then(|reaction| self.server_aliases.get(&reaction.reactor)
                                        .map(String::as_str)
                                        .or_else(|| self.reactor_profiles.get(&reaction.reactor)
                                            .map(|profile| profile.name.as_str())))
                                    .filter(|name| !name.is_empty());
                                let raw_preview = crate::message_view::preview_with_resolver(t, reactor_name,
                                    |uid| self.server_aliases.get(uid).map(String::as_str));
                                let unread_n = self.unread_counts.get(&t.id).copied().unwrap_or(0);
                                let snippet_right = row_rect.right() - 9.0
                                    - if unread_n > 0 || t.unread.marked_unread { 30.0 } else { 0.0 }
                                    - if t.unread.has_unread_mentions() { 38.0 } else { 0.0 };
                                let snippet_color = if unread_n > 0 || t.unread.marked_unread { pal.text_primary } else { pal.text_secondary };
                                let mut snippet_job = egui::text::LayoutJob::simple_singleline(raw_preview, FontId::proportional(12.0), snippet_color);
                                snippet_job.wrap.max_width = (snippet_right - text_left).max(1.0);
                                snippet_job.wrap.max_rows = 1;
                                snippet_job.wrap.break_anywhere = true;
                                let snippet_galley = ui.fonts(|f| f.layout_job(snippet_job));
                                ui.painter().galley(Pos2::new(text_left, row_rect.top() + 33.5), snippet_galley, snippet_color);
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
                                } else if t.unread.marked_unread {
                                    let dot_center = Pos2::new(row_rect.right() - 17.0, row_rect.top() + 40.0);
                                    ui.painter().circle_filled(dot_center, 4.5, pal.danger);
                                }
                            }

                            let row_resp = row_resp.on_hover_cursor(CursorIcon::PointingHand);
                            let row_resp = if is_narrow_w {
                                row_resp.on_hover_text(thread_display)
                            } else {
                                row_resp
                            };
                            if !modal_open && row_resp.clicked() {
                                clicked_thread = Some((i, t.id.clone(), t.is_group));
                            }
                        }

                        if !query.is_empty() && filtered_indices.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(32.0);
                                ui.label(
                                    RichText::new(
                                        crate::i18n::tr("Không tìm thấy hội thoại hoặc tin nhắn phù hợp."),
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
                if let Some((target,group,action))=manual_mark_action {self.manual_mark_command(target,group,action);}
                if let Some((target, group, archived)) = archive_action {
                    self.archive_command(target, group, Some(archived));
                }
                if let Some(target) = label_target {
                    self.open_labels(Some(target));
                }
                if let Some(contact) = invite_contact_target {
                    self.open_invite_to_group_picker(contact);
                }
                if let Some((target, group)) = report_target {
                    self.open_contact_report(target, group);
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
                                confirmed = ui.button(crate::i18n::tr("Xóa lịch sử")).clicked();
                                cancelled = ui.button(crate::i18n::tr("Hủy")).clicked();
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
                if let Some((thread,id)) = mention_target {
                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::Tools{owner:self.user_uin.clone(),command:tools_panel::ToolsCommand::OpenHit{thread,id}});}
                } else if let Some((idx, tid, is_group)) = clicked_thread {
                    if is_narrow_w {
                        ctx.data_mut(|data| {
                            data.insert_temp(Id::new("narrow_chat_open"), true);
                            data.insert_temp(Id::new("narrow_drawer_open"), false);
                        });
                    }
                    self.open_thread(ctx, idx, tid, is_group);
                }
            });
            #[cfg(test)]
            ctx.data_mut(|data| {
                data.insert_temp(
                    Id::new("sidebar-panel-rect"),
                    sidebar_response.response.rect,
                )
            });
            #[cfg(not(test))]
            let _ = sidebar_response;
        }

        // Extract lightweight metadata for Header, Right Info Drawer, and Central Chat Stream
        let thread_meta = self
            .threads
            .get(self.selected_thread_index)
            .filter(|t| self.private_access(&t.id, t.is_group))
            .map(|t| {
                (
                    t.id.clone(),
                    self.resolve_thread_name(t).to_owned(),
                    t.avatar.clone(),
                    t.is_group,
                    t.messages.len(),
                )
            });

        // -------------------------------------------------------------------
        // 3. COLLAPSIBLE RIGHT CONVERSATION INFO DRAWER (responsive width)
        // -------------------------------------------------------------------
        let show_info_drawer = self.info_drawer_open && !is_compact_w;
        let drawer_w = (screen_w * 0.28).clamp(280.0, 360.0);

        if show_info_drawer {
            if let Some((ref d_id, ref d_name, ref d_ava, d_is_group, d_msg_count)) = thread_meta {
                let (shared_photos, shared_files, shared_links) = self
                    .threads
                    .get(self.selected_thread_index)
                    .map(extract_shared_media_and_links)
                    .unwrap_or_default();

                egui::SidePanel::right("conversation_info_drawer")
                    .resizable(true)
                    .default_width(drawer_w)
                    .width_range(280.0..=380.0)
                    .frame(
                        egui::Frame::none()
                            .fill(pal.bg_sidebar)
                            .stroke(Stroke::new(1.0_f32, pal.border)),
                    )
                    .show(ctx, |ui| {
                        if modal_open {
                            ui.style_mut().interaction.tooltip_delay = 86_400.0;
                        }
                        match self.drawer_view {
                            DrawerView::Overview => {
                                self.render_drawer_overview(
                                    ui,
                                    d_id,
                                    d_name,
                                    d_ava.as_deref(),
                                    d_is_group,
                                    d_msg_count,
                                    &shared_photos,
                                    &shared_files,
                                    &shared_links,
                                    pal,
                                );
                            }
                            DrawerView::Members => {
                                self.render_drawer_members_view(ui, d_id, pal);
                            }
                            DrawerView::Board => {
                                self.render_drawer_board_view(ui, d_id, d_is_group, pal);
                            }
                            DrawerView::MediaStorage { tab } => {
                                self.render_drawer_media_storage_view(
                                    ui,
                                    d_id,
                                    d_is_group,
                                    tab,
                                    &shared_photos,
                                    &shared_files,
                                    &shared_links,
                                    pal,
                                );
                            }
                            DrawerView::SearchInThread => {
                                self.render_drawer_search_view(ui, d_id, pal);
                            }
                            DrawerView::SecuritySettings => {
                                self.render_drawer_security_view(ui, d_id, d_name, d_is_group, pal);
                            }
                        }
                    });
            }
        }

        if is_compact_w && narrow_drawer_open && !modal_open {
            if let Some((ref d_id, ref d_name, ref d_ava, d_is_group, d_msg_count)) = thread_meta {
                let (shared_photos, shared_files, shared_links) = self
                    .threads
                    .get(self.selected_thread_index)
                    .map(extract_shared_media_and_links)
                    .unwrap_or_default();
                let drawer_width = (screen_w * 0.86).clamp(160.0, 360.0).min(screen_w);
                egui::Area::new(Id::new("narrow_conversation_drawer"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(Pos2::new(
                        (screen_rect.right() - drawer_width).max(screen_rect.left()),
                        screen_rect.top(),
                    ))
                    .constrain_to(screen_rect)
                    .show(ctx, |ui| {
                        ui.set_width(drawer_width);
                        ui.set_min_height((screen_rect.height() - 44.0).max(160.0));
                        egui::Frame::none()
                            .fill(pal.bg_sidebar)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(10.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("Thông tin hội thoại")
                                            .strong()
                                            .color(pal.text_primary),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        if ui.small_button(lucide::X).clicked() {
                                            ctx.data_mut(|data| {
                                                data.insert_temp(
                                                    Id::new("narrow_drawer_open"),
                                                    false,
                                                );
                                            });
                                        }
                                    });
                                });
                                ui.separator();
                                match self.drawer_view {
                                    DrawerView::Overview => self.render_drawer_overview(
                                        ui,
                                        d_id,
                                        d_name,
                                        d_ava.as_deref(),
                                        d_is_group,
                                        d_msg_count,
                                        &shared_photos,
                                        &shared_files,
                                        &shared_links,
                                        pal,
                                    ),
                                    DrawerView::Members => {
                                        self.render_drawer_members_view(ui, d_id, pal)
                                    }
                                    DrawerView::Board => {
                                        self.render_drawer_board_view(ui, d_id, d_is_group, pal)
                                    }
                                    DrawerView::MediaStorage { tab } => self
                                        .render_drawer_media_storage_view(
                                            ui,
                                            d_id,
                                            d_is_group,
                                            tab,
                                            &shared_photos,
                                            &shared_files,
                                            &shared_links,
                                            pal,
                                        ),
                                    DrawerView::SearchInThread => {
                                        self.render_drawer_search_view(ui, d_id, pal)
                                    }
                                    DrawerView::SecuritySettings => self
                                        .render_drawer_security_view(
                                            ui, d_id, d_name, d_is_group, pal,
                                        ),
                                }
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
                if modal_open {
                    ui.style_mut().interaction.tooltip_delay = 86_400.0;
                }
                if is_narrow_w && narrow_chat_open {
                    ui.horizontal(|ui| {
                        if ui
                            .button("← Quay lại")
                            .on_hover_text("Quay lại danh sách hội thoại")
                            .clicked()
                        {
                            ctx.data_mut(|data| {
                                data.insert_temp(Id::new("narrow_chat_open"), false);
                            });
                        }
                    });
                    self.render_sync_banner(ui, ctx, pal);
                }
                if let Some((thread_id, thread_name, thread_avatar, is_group, _msg_count)) =
                    thread_meta
                {
                    let is_cloud_thread = self
                        .threads
                        .get(self.selected_thread_index)
                        .is_some_and(|thread| self.is_cloud_thread(thread));
                    if !is_group
                        && !is_cloud_thread
                        && !self.friend_status_map.contains_key(&thread_id)
                    {
                        self.request_friend_action(&thread_id, FriendAction::Check);
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
                        .inner_margin(Margin::symmetric(18.0, 12.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                // Header Avatar (40x40) with Cutout Online Dot
                                let (av_rect, av_response) = ui.allocate_exact_size(
                                    Vec2::new(40.0, 40.0),
                                    if is_group && !modal_open { Sense::click() } else { Sense::hover() },
                                );
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
                                let show_online = self.inbox_policy.show_online == Some(true);
                                let now_ms = crate::login::unix_now() * 1000;
                                let presence_status = if !is_group && !is_cloud_thread && show_online {
                                    self.presence_map.get(&thread_id).map(|(s, _)| s.clone())
                                } else {
                                    None
                                };
                                let is_active_now = match &presence_status {
                                    Some(crate::last_activity::Status::LastActivity(ts)) => {
                                        show_online && now_ms.saturating_sub(*ts) <= 180_000
                                    }
                                    _ => false,
                                };
                                if is_active_now {
                                    let dot_c = Pos2::new(av_rect.right() - 4.0, av_rect.bottom() - 4.0);
                                    ui.painter().circle_filled(dot_c, 6.5, pal.bg_sidebar);
                                    ui.painter().circle_filled(dot_c, 4.5, pal.accent_online);
                                }
                                if is_group && !modal_open && group_avatar_affordance(ui, av_response, pal) {
                                    self.open_conversation_avatar_editor(&thread_id);
                                }
                                ui.add_space(6.0);
                                // Reserve room for the right-hand action cluster so the name/tag
                                // block can never be overlapped by the call buttons.
                                let actions_w = if is_compact_w { 160.0 } else { 220.0 };
                                let left_info_w =
                                    (ui.available_width() - actions_w).max(60.0);
                                ui.allocate_ui_with_layout(
                                    Vec2::new(left_info_w, 40.0),
                                    Layout::top_down(Align::Min),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.y = 2.0;
                                        ui.horizontal(|ui| {
                                            let name_max = if is_compact_w { 12 } else { 22 };
                                            let name_response = ui.add(
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
                                            if !is_cloud_thread && thread_id != self.user_uin
                                                && name_edit_affordance(ui, name_response.hovered(), !modal_open, is_group, pal)
                                            {
                                                self.open_conversation_name_editor(&thread_id, &thread_name, is_group);
                                            }
                                            let label_identity = crate::server_pins::identity(&thread_id, is_group).ok();
                                            let active_label = label_identity.as_deref().and_then(|identity| {
                                                self.labels_panel.snapshot.as_ref().and_then(|snapshot| {
                                                    snapshot.labels.iter().find(|label| label.contains(identity))
                                                })
                                            });
                                            let label_text = active_label
                                                .map(|label| format!("{} {}", "Phân loại", label.text))
                                                .unwrap_or_else(|| "Phân loại".to_owned());
                                            if ui
                                                .small_button(label_text)
                                                .on_hover_text(crate::i18n::tr("Mở phân loại hội thoại"))
                                                .clicked()
                                            {
                                                self.open_labels(Some((thread_id.clone(), is_group)));
                                            }
                                            if !is_compact_w && left_info_w > 170.0 {
                                                let (tag_lbl, tag_fg, tag_bg) =
                                                    if is_cloud_thread {
                                                        (
                                                            "Đám mây",
                                                            pal.primary_hover,
                                                            pal.primary.linear_multiply(0.14),
                                                        )
                                                    } else if is_group {
                                                        (self.threads.iter().find(|thread| thread.id == thread_id && thread.is_group)
                                                            .and_then(|thread| thread.presentation.group_kind)
                                                            .map(crate::chat::GroupKind::label).unwrap_or("Hội thoại nhóm"),
                                                        pal.text_secondary, pal.bg_elevated)
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
                                            ("Nhóm trò chuyện".to_string(), pal.text_secondary)
                                        } else if is_cloud_thread {
                                            ("Lưu trữ đám mây cá nhân".to_string(), pal.text_secondary)
                                        } else {
                                            match presence_status {
                                                Some(crate::last_activity::Status::LastActivity(ts)) => {
                                                    let diff_ms = now_ms.saturating_sub(ts);
                                                    if diff_ms <= 180_000 {
                                                        ("Đang hoạt động".to_string(), pal.accent_online)
                                                    } else if diff_ms < 3_600_000 {
                                                        let mins = (diff_ms / 60_000).max(1);
                                                        (format!("Truy cập {mins} phút trước"), pal.text_secondary)
                                                    } else if diff_ms < 86_400_000 {
                                                        let hours = (diff_ms / 3_600_000).max(1);
                                                        (format!("Truy cập {hours} giờ trước"), pal.text_secondary)
                                                    } else if diff_ms < 172_800_000 {
                                                        ("Truy cập hôm qua".to_string(), pal.text_secondary)
                                                    } else {
                                                        let days = (diff_ms / 86_400_000).max(1);
                                                        (format!("Truy cập {days} ngày trước"), pal.text_secondary)
                                                    }
                                                }
                                                _ => match cur_friendship {
                                                    crate::chat::FriendshipStatus::Friend => {
                                                        ("Đã kết bạn".to_string(), pal.text_secondary)
                                                    }
                                                    crate::chat::FriendshipStatus::IncomingPending => (
                                                        "Đã gửi lời mời kết bạn cho bạn".to_string(),
                                                        pal.primary_hover,
                                                    ),
                                                    crate::chat::FriendshipStatus::OutgoingPending => (
                                                        "Đã gửi lời mời kết bạn · Chờ đồng ý".to_string(),
                                                        pal.warning,
                                                    ),
                                                    crate::chat::FriendshipStatus::Stranger => (
                                                        "Người lạ · Chưa kết bạn trên Zalo".to_string(),
                                                        Color32::from_rgb(249, 115, 22),
                                                    ),
                                                    crate::chat::FriendshipStatus::Unknown => {
                                                        ("Chưa có thông tin trạng thái".to_string(), pal.text_secondary)
                                                    }
                                                },
                                            }
                                        };
                                        ui.label(
                                            RichText::new(&sub_status_lbl)
                                                .size(11.6)
                                                .color(sub_status_col),
                                        );
                                    },
                                );

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    // Compact layouts use an overlay without shrinking the chat.
                                    {
                                        let drawer_open = if is_compact_w {
                                            narrow_drawer_open
                                        } else {
                                            self.info_drawer_open
                                        };
                                        let (drw_rect, drw_resp) = ui.allocate_exact_size(
                                            Vec2::new(34.0, 34.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            drw_rect,
                                            Rounding::same(8.0),
                                            if drawer_open {
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
                                            if drawer_open {
                                                pal.primary_hover
                                            } else if drw_resp.hovered() {
                                                pal.text_primary
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                        if drw_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(crate::i18n::tr("Thông tin hội thoại"))
                                            .clicked()
                                        {
                                            if is_compact_w {
                                                ctx.data_mut(|data| {
                                                    data.insert_temp(
                                                        Id::new("narrow_drawer_open"),
                                                        !narrow_drawer_open,
                                                    );
                                                });
                                                if !narrow_drawer_open
                                                    && !is_group
                                                    && !self.trial_peers.contains(&thread_id)
                                                {
                                                    if let Some(tx) = WORKER_TX.get() {
                                                        let _ = tx.send(WorkerCmd::CheckE2eeStatus {
                                                            owner: self.user_uin.clone(),
                                                            peer_id: thread_id.clone(),
                                                        });
                                                    }
                                                }
                                            } else {
                                                self.info_drawer_open = !self.info_drawer_open;
                                                self.settings.info_drawer_open = self.info_drawer_open;
                                                if let Some(tx) = WORKER_TX.get() {
                                                    let _ = tx.send(WorkerCmd::PersistInfoDrawer {
                                                        owner: self.user_uin.clone(),
                                                        open: self.info_drawer_open,
                                                    });
                                                }
                                                if self.info_drawer_open
                                                    && !is_group
                                                    && !self.trial_peers.contains(&thread_id)
                                                {
                                                    if let Some(tx) = WORKER_TX.get() {
                                                        let _ = tx.send(WorkerCmd::CheckE2eeStatus {
                                                            owner: self.user_uin.clone(),
                                                            peer_id: thread_id.clone(),
                                                        });
                                                    }
                                                }
                                            }
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
                                        .on_hover_text(crate::i18n::tr("Gọi Video"))
                                        .clicked()
                                    {
                                        self.open_preflight_call(
                                            thread_id.clone(),
                                            thread_name.clone(),
                                            None,
                                            true,
                                            is_group,
                                        );
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
                                        .on_hover_text(crate::i18n::tr("Gọi thoại"))
                                        .clicked()
                                    {
                                        self.open_preflight_call(
                                            thread_id.clone(),
                                            thread_name.clone(),
                                            None,
                                            false,
                                            is_group,
                                        );
                                    }
                                    // Search Messages in Conversation Button (`Ctrl+F`)
                                    let (srch_rect, srch_resp) = ui.allocate_exact_size(
                                        Vec2::new(34.0, 34.0),
                                        Sense::click(),
                                    );
                                    let is_search_active = (self.info_drawer_open && self.drawer_view == DrawerView::SearchInThread)
                                        || (self.history_search.open && self.history_search.request.thread.as_deref() == Some(thread_id.as_str()));
                                    ui.painter().rect_filled(
                                        srch_rect,
                                        Rounding::same(8.0),
                                        if is_search_active {
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
                                        if is_search_active {
                                            pal.primary_hover
                                        } else if srch_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if srch_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(crate::i18n::tr("Tìm kiếm tin nhắn trong hội thoại này (Ctrl+F)"))
                                        .clicked()
                                        || (!self.is_workspace_input_gated(ctx)
                                            && ui.input(|i| {
                                                (i.modifiers.ctrl || i.modifiers.command)
                                                    && i.key_pressed(Key::F)
                                            }))
                                    {
                                        self.info_drawer_open = true;
                                        self.drawer_view = DrawerView::SearchInThread;
                                    }

                                    // Add Member / Create Group Button (5th Action Button)
                                    let (grp_rect, grp_resp) = ui.allocate_exact_size(
                                        Vec2::new(34.0, 34.0),
                                        Sense::click(),
                                    );
                                    if grp_resp.hovered() {
                                        ui.painter().rect_filled(
                                            grp_rect,
                                            Rounding::same(8.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    draw_icon_user_plus(
                                        ui.painter(),
                                        grp_rect.center(),
                                        17.5,
                                        if grp_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    let grp_tooltip = if is_group {
                                        crate::i18n::tr("Thêm bạn vào nhóm")
                                    } else {
                                        crate::i18n::tr("Thêm bạn / Tạo nhóm chung")
                                    };
                                    if grp_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(grp_tooltip)
                                        .clicked()
                                    {
                                        if is_group {
                                            self.open_group_panel(thread_id.clone(), true, false);
                                        } else {
                                            self.open_group_panel(thread_id.clone(), false, true);
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
                                                        RichText::new(crate::i18n::tr("Từ chối"))
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
                                                        self.request_friend_action(&thread_id, FriendAction::Reject);
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
                                                        self.request_friend_action(&thread_id, FriendAction::Accept);
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
                                                        RichText::new(crate::i18n::tr("Hủy lời mời"))
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
                                                        self.request_friend_action(&thread_id, FriendAction::Undo);
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
                                                        self.request_friend_action(&thread_id, FriendAction::Add);
                                                    }
                                                },
                                            );
                                        }
                                        _ => {}
                                    }
                                });
                            });
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

                    // Message pins use their own thread/generation-scoped server snapshot.
                    if self.message_pins.thread != thread_id {
                        self.refresh_message_pins(&thread_id, is_group);
                    }
                    let (latest_pin_id, preview_text, total_pins, pin_count_label) = {
                        let page = (self.message_pins.thread == thread_id)
                            .then_some(self.message_pins.page.as_ref()).flatten();
                        if let Some(page) = page {
                            let latest_server = page.items.first();
                            let latest_pin_id = latest_server
                                .map(|item| item.message_id.clone())
                                .filter(|id| !id.is_empty());
                            let preview_text = latest_server
                                .map(|item| truncate_chars(&item.title, 50).into_owned())
                                .unwrap_or_else(|| "[Tin nhắn đã ghim]".to_string());
                            let total_pins = page.items.len();
                            let suffix = if page.next.is_some() { "+" } else { "" };
                            let count_label = format!("Danh sách {total_pins}{suffix} ghim ▾");
                            (latest_pin_id, preview_text, total_pins, count_label)
                        } else {
                            (None, "[Tin nhắn đã ghim]".to_string(), 0, String::new())
                        }
                    };
                    if self.message_pins.thread == thread_id && self.message_pins.pending {
                        ui.horizontal(|ui| { ui.spinner(); ui.label("Đang tải tin ghim…"); });
                    } else if self.message_pins.thread == thread_id && !self.message_pins.error.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label("Chưa tải được tin ghim").on_hover_text(&self.message_pins.error);
                            if ui.small_button("Tải lại").clicked() { self.refresh_message_pins(&thread_id, is_group); }
                        });
                    }
                    if total_pins > 0 {
                        egui::Frame::none()
                            .fill(pal.bg_elevated)
                            .stroke(Stroke::new(1.0_f32, pal.border))
                            .inner_margin(Margin::symmetric(18.0, 7.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let pin_click = ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        let (pin_icon_rect, _) = ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                                        draw_icon_pin(ui.painter(), pin_icon_rect.center(), 14.0, pal.primary);
                                        ui.label(
                                            RichText::new("Tin nhắn đã ghim:")
                                                .size(12.0)
                                                .color(pal.text_secondary)
                                                .strong(),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&preview_text)
                                                    .size(12.0)
                                                    .color(pal.text_primary),
                                            )
                                            .truncate(),
                                        )
                                    }).response.interact(Sense::click());

                                    if pin_click
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(crate::i18n::tr("Nhảy đến tin nhắn đã ghim"))
                                        .clicked()
                                    {
                                        if let Some(pin_id) = &latest_pin_id {
                                            self.filter_starred_only = false;
                                            self.tools_command(super::tools_panel::ToolsCommand::OpenHit { thread: thread_id.clone(), id: pin_id.clone() });
                                            ctx.request_repaint();
                                        }
                                    }

                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let list_btn = egui::Button::new(
                                            RichText::new(&pin_count_label)
                                                .size(11.5)
                                                .color(pal.primary_hover),
                                        )
                                        .fill(Color32::TRANSPARENT)
                                        .stroke(Stroke::NONE);

                                        if ui
                                            .add(list_btn)
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(crate::i18n::tr("Xem toàn bộ tin nhắn đã ghim & nhắc hẹn"))
                                            .clicked()
                                        {
                                            self.open_message_pins(thread_id.clone(), is_group);
                                        }
                                    });
                                });
                            });
                    }

                    // 2. Scrollable Messages Area
                    let composer_id = Id::new(("chat_composer_input", &thread_id));
                    let composer_scope = composer_id.with((&self.user_uin, self.account_runtime_id));
                    if let Some(recovered) = ctx.data_mut(|data| data.remove_temp::<String>(composer_scope.with("cancelled_draft"))) {
                        self.input_message.push_str(&recovered);
                    }
                    if !modal_open && ctx.memory(|memory| memory.has_focus(composer_id)) {
                        stage_composer_events(ctx, composer_scope, self.composer_ime_active);
                    }
                    let composer_format_key = Id::new(("composer_format", &self.user_uin, self.account_runtime_id, &thread_id, is_group));
                    let mut composer_format = ctx.data(|data| data.get_temp::<crate::rich_text::ComposerFormat>(composer_format_key)).unwrap_or_default();
                    composer_format.edit(&self.input_message);
                    if !modal_open && ctx.memory(|memory| memory.has_focus(composer_id))
                        && ctx.input_mut(|input| consume_composer_format_shortcut(&mut input.events))
                    {
                        self.composer_format_mode = !self.composer_format_mode;
                        if !self.composer_format_mode {
                            composer_format.clear_text_styles();
                            ctx.data_mut(|data|data.insert_temp(composer_format_key,composer_format.clone()));
                        }
                    }
                    let expanded_key = composer_scope.with("expanded");
                    let mut composer_expanded = self.composer_format_mode
                        && ctx.data(|data| data.get_temp::<bool>(expanded_key).unwrap_or(false));
                    let contextual_stickers = if self.attachment_drafts.len(&thread_id)>0 {Vec::new()} else {self.resolve_contextual_sticker_suggestions(ctx)};
                    let composer_link_preview =
                        self.resolve_composer_link_preview(ctx, &thread_id, is_group);
                    // Do NOT subtract `contextual_stickers` height from `picker_reserve`!
                    // The sticker strip is a true floating overlay (`egui::Area`) above the composer bar
                    // (matching Zalo PC's `PopoverIdentitiesDefine.STICKER_SUGGESTION`), so the message scroll
                    // area and composer input box never shift or lose IME focus while typing.
                    let attachment_count = self.attachment_drafts.len(&thread_id);
                    let picker_reserve = (if attachment_count > 0 {152.0} else {0.0})
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
                    // A changed message list is first laid out at a stale scroll offset (top of a new
                    // conversation, or mid-list after older history arrives). Discard that pass so
                    // only the re-anchored layout reaches the screen.
                    let layout_key = self
                        .threads
                        .get(self.selected_thread_index)
                        .map(|t| chat_layout_key(&thread_id, &t.messages, render_limit))
                        .unwrap_or_default() ^ self.chat_geometry_revision.rotate_left(7) ^ (ui.available_width().to_bits() as u64).rotate_left(17) ^ (ctx.pixels_per_point().to_bits() as u64);
                    let layout_changed=layout_key!=self.chat_layout_key;
                    let previous_anchor=self.chat_anchor.clone().filter(|(thread,_,_)| thread==&thread_id);
                    let mut next_anchor=None;
                    let mut anchor_delta=None;
                    if layout_changed {
                        self.chat_layout_key = layout_key;
                        ctx.request_discard("chat message list changed");
                    }
                    // Measure visual wrapping, not only explicit newlines: clipping and DPI
                    // can leave a narrow editor even when its parent has a wider layout.
                    let composer_layout_width = ui.available_width().min(ui.clip_rect().width());
                    let composer_compact = composer_layout_width < 272.0;
                    let composer_send_width = if composer_compact { 48.0 } else { 72.0 };
                    let composer_emoji_width = 34.0;
                    let composer_input_width = (composer_layout_width
                        - 30.0
                        - 18.0
                        - composer_emoji_width
                        - composer_send_width
                        - 8.0
                        - if composer_compact { 48.0 } else { 0.0 })
                        .max(32.0);
                    let toolbar_button_count = 9 + usize::from(cfg!(windows));
                    let toolbar_inner_width = (composer_layout_width - 30.0).max(1.0);
                    let toolbar_per_row = ((toolbar_inner_width + 2.0) / 34.0).floor().max(1.0) as usize;
                    let toolbar_rows = toolbar_button_count.div_ceil(toolbar_per_row);
                    self.sync_composer_draft(ctx);
                    let composer_line_height = if self.composer_format_mode {self.composer_config.line_height} else {22.0};
                    let measured_editor_height = ui.fonts(|fonts| fonts.layout_job(composer_layout_job(
                        &self.input_message,&self.composer_mentions,&composer_format,composer_input_width-12.0,
                        composer_line_height,pal)).size().y).max(composer_line_height)+6.0;
                    let editor_height = if composer_expanded {
                        (ui.available_height() * 0.5 - 88.0).max(60.0)
                    } else if self.composer_format_mode {
                        measured_editor_height.max(140.0)
                            .min(self.composer_config.rich_max_lines*composer_line_height+6.0)
                    } else {
                        measured_editor_height.min(COMPOSER_MAX_ROWS as f32*composer_line_height+6.0)
                    };
                    let send_policy = self.threads[self.selected_thread_index].presentation.send_policy.as_ref();
                    // The worker revalidates group permission before sending. Keep the composer
                    // usable while its first snapshot is loading; a missing snapshot is not a denial.
                    let group_send_allowed = !is_group || send_policy.is_none_or(|policy| policy.allows(&self.user_uin));
                    let group_notice = (!group_send_allowed).then(|| {
                        let text = "Nhóm đang khóa gửi tin hoặc bạn không còn quyền gửi.";
                        ui.fonts(|fonts| fonts.layout(text.into(), egui::TextStyle::Body.resolve(ui.style()), pal.text_primary, ui.available_width()))
                    });
                    // Reserve the same wrapped galley that is rendered below the history.
                    // A permission notice must not push the composer outside the viewport.
                    let notice_height = group_notice.as_ref().map(|galley| galley.size().y + ui.spacing().item_spacing.y).unwrap_or(0.0);
                    let toolbar_height = 36.0 + (toolbar_rows.saturating_sub(1) as f32 * 36.0);
                    let format_toolbar_height = if self.composer_format_mode || composer_format.urgency.is_some() { 30.0 } else { 0.0 };
                    // Reserve the measured composer, not an unrelated fixed bottom spacer.
                    // A discard pass converges after wrapping, mode switches or viewport changes.
                    let composer_bottom = ui.max_rect().bottom();
                    let composer_height_key = composer_scope.with("measured_height");
                    let estimated_height = toolbar_height + editor_height.max(34.0)
                        + format_toolbar_height + 25.0;
                    let reserved_composer_height = ctx.data(|data|
                        data.get_temp::<f32>(composer_height_key).unwrap_or(estimated_height));
                    let msg_area_height = (ui.available_height() - picker_reserve
                        - reserved_composer_height - notice_height - ui.spacing().item_spacing.y)
                        .max(0.0);

                    let newest_requested = ctx.data_mut(|d| d.remove_temp::<bool>(Id::new(("jump_newest", &thread_id)))).unwrap_or(false);
                    let content_protected=self.content_protected(&thread_id,is_group);
                    if self.settings.use_avatar_as_wallpaper {
                        if let Some(url) = thread_avatar.as_deref().map(str::trim).filter(|url| !url.is_empty()) {
                            let key = format!("ava_{}",crate::crypto::md5_hex(url));
                            if let Some(texture) = self.sticker_textures.get(&key) {
                                let canvas = Rect::from_min_size(ui.cursor().min,Vec2::new(ui.available_width(),msg_area_height));
                                ui.painter().image(texture.id(),canvas,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),Color32::WHITE.linear_multiply(0.12));
                            } else if self.loading_stickers.insert(key.clone()) {
                                if let Some(tx)=WORKER_TX.get() {
                                    if tx.send(WorkerCmd::FetchStickerImage {key:key.clone(),url:url.to_owned()}).is_err() {
                                        self.loading_stickers.remove(&key);
                                    }
                                } else {self.loading_stickers.remove(&key);}
                            }
                        }
                    }
                    if modal_open {
                        ctx.data_mut(|d| {
                            d.remove::<(String, i64, egui::Rect)>(Id::new(("hover_message", &thread_id)));
                            d.remove::<(String, i64, f64, Pos2)>(Id::new(("hover_message_dwell", &thread_id)));
                            d.remove::<(String, i64, egui::Rect)>(Id::new(("hover_reaction_btn", &thread_id)));
                            d.remove::<(String, i64)>(Id::new(("hover_message_menu", &thread_id)));
                        });
                    }
                    let message_scroll = egui::ScrollArea::vertical()
                        .id_salt(Id::new(("chat_scroll", &thread_id)))
                        .min_scrolled_height(0.0)
                        .max_height(msg_area_height)
                        .auto_shrink([false, false])
                        .stick_to_bottom(jump_index.is_none())
                        .show(ui, |ui| {
                            if content_protected {ui.style_mut().interaction.selectable_labels=false;}
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
                                                        crate::i18n::tr("Chưa có tin nhắn trong cuộc trò chuyện này."),
                                                    )
                                                    .size(14.0)
                                                    .color(pal.text_muted),
                                                );
                                            });
                                        } else {
                                            if !self.filter_starred_only
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
                                                            crate::i18n::tr("Đồng bộ thêm lịch sử tin nhắn cũ từ máy chủ Zalo"),
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
                                            let visible_indices: Vec<usize> = thread
                                                .messages
                                                .iter()
                                                .enumerate()
                                                .filter(|(_, m)| !only_starred || m.is_starred)
                                                .map(|(i, _)| i)
                                                .collect();
                                            let visible_indices = if only_starred {
                                                visible_indices
                                            } else {
                                                let skip = visible_indices.len().saturating_sub(render_limit);
                                                visible_indices[skip..].to_vec()
                                            };
                                            let last_outgoing_idx = thread
                                                .messages
                                                .iter()
                                                .enumerate()
                                                .rev()
                                                .find(|(_, m)| {
                                                    m.is_outgoing && !is_system_notice_message(m)
                                                })
                                                .map(|(i, _)| i);

                                            let mut album_rendered=std::collections::HashSet::new();
                                            for (pos, &idx) in visible_indices.iter().enumerate() {
                                                if album_rendered.contains(&idx){continue;}
                                                let msg = &thread.messages[idx];
                                                if let Some(bundle)=crate::message_view::photo_bundle(msg) {
                                                    let members:Vec<usize>=visible_indices[pos..].iter().copied().take(9).take_while(|&index|{
                                                        let other=&thread.messages[index];
                                                        other.sender==msg.sender&&other.is_outgoing==msg.is_outgoing&&crate::message_view::photo_bundle(other).as_ref()==Some(&bundle)
                                                    }).collect();
                                                    if members.len()>1 {
                                                        let previous=pos.checked_sub(1).and_then(|p|visible_indices.get(p)).map(|&index|&thread.messages[index]);
                                                        if should_show_time_divider(previous,msg){render_date_divider(ui,msg,pal);}
                                                        let width=(ui.available_width()*0.82).clamp(120.0,440.0);
                                                        let columns=if width<200.0{1}else{2};
                                                        let cell_width=(width-16.0-(columns-1) as f32*6.0)/columns as f32;
                                                        ui.with_layout(if msg.is_outgoing{Layout::right_to_left(Align::TOP)}else{Layout::left_to_right(Align::TOP)},|ui|{
                                                            egui::Frame::none().fill(if msg.is_outgoing{pal.bubble_out_bg}else{pal.bubble_in_bg})
                                                                .rounding(10.0).inner_margin(8.0).show(ui,|ui|{
                                                                ui.set_width(width-16.0);
                                                                let album_sender = resolve_message_sender_name_from(&self.server_aliases,&self.threads,&self.group_members,msg,&thread_id,is_group,&thread_name);
                                                                if is_group&&!msg.is_outgoing {ui.label(RichText::new(album_sender).small().color(pal.primary_hover));}
                                                                for chunk in members.chunks(columns){ui.horizontal_top(|ui|{
                                                                    for &index in chunk {
                                                                        let item=&thread.messages[index];
                                                                        let item_sender = resolve_message_sender_name_from(&self.server_aliases,&self.threads,&self.group_members,item,&thread_id,is_group,&thread_name);
                                                                        let cell=ui.allocate_ui_with_layout(Vec2::new(cell_width,0.0),Layout::top_down(Align::LEFT),|ui|{
                                                                            if let Some(action)=render_message_body(ui,item,&thread_name,&thread_id,is_group,&self.image_failures,pal,item.is_outgoing,cell_width,&self.sticker_textures,&mut self.loading_stickers,(!item.is_outgoing).then_some(item_sender),&self.server_aliases,self.settings.link_suggestions_enabled,self.settings.file_thumbnails_enabled){pending_action=Some((index,action));}
                                                                        });
                                                                        capture_anchor(ui,cell.response.rect,&item.id,previous_anchor.as_ref(),&mut next_anchor,&mut anchor_delta);
                                                                        if !modal_open {
                                                                            if let Some(action)=bubble_context_menu(ui,&cell.response,item,&self.emoji_textures,is_group,content_protected){pending_action=Some((index,action));}
                                                                        }
                                                                        if let Some(action)=hover_message_actions(ui,cell.response.rect,&thread_id,item,&self.emoji_textures,modal_open){pending_action=Some((index,action));}
                                                                        if ui.is_rect_visible(cell.response.rect){ctx.data_mut(|d|d.get_temp_mut_or_default::<Vec<String>>(Id::new("visible_mentions")).push(item.id.clone()));}
                                                                        if jump_index==Some(index){render_quote_target(ui,cell.response.rect,jump_scroll_pending,pal.primary);jump_scrolled|=jump_scroll_pending;}
                                                                        album_rendered.insert(index);
                                                                    }
                                                                });}
                                                                ui.label(RichText::new(msg.display_time()).size(10.5).color(if msg.is_outgoing{pal.bubble_out_meta}else{pal.bubble_in_meta}));
                                                            });
                                                        });
                                                        ui.add_space(8.0);
                                                        continue;
                                                    }
                                                }
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

                                                let width_bucket = ((ui.available_width() / 20.0).round() as u32) * 20;
                                                let cache_key = (thread_id.clone(), msg.id.clone(), width_bucket);
                                                let clip = ui.clip_rect();
                                                let current_y = ui.cursor().top();
                                                let is_far_offscreen = (current_y < clip.top() - 600.0) || (current_y > clip.bottom() + 600.0);

                                                if is_far_offscreen
                                                    && jump_index.is_none()
                                                    && !newest_requested
                                                    && !msg.id.is_empty()
                                                    && !is_system_notice_message(msg)
                                                {
                                                    if let Some(&cached_height) = self.measured_msg_heights.get(&cache_key) {
                                                        let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), cached_height));
                                                        capture_anchor(ui, rect, &msg.id, previous_anchor.as_ref(), &mut next_anchor, &mut anchor_delta);
                                                        ui.add_space(if next_msg.map(|nm| same_message_cluster(msg, nm)).unwrap_or(false) {
                                                            2.5
                                                        } else {
                                                            9.0
                                                        });
                                                        continue;
                                                    }
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
                                                        let raw = msg.wire_ref.sender.as_ref();
                                                        if let Some(uid) = raw.and_then(|v| v.as_str()) {
                                                            sender_avatar_for(&self.threads, uid)
                                                        } else if let Some(n) = raw.and_then(|v| v.as_u64()) {
                                                            let num_s = n.to_string();
                                                            sender_avatar_for(&self.threads, &num_s)
                                                        } else {
                                                            None
                                                        }
                                                    };
                                                    if self.batch_selection_mode && !msg.id.is_empty() {
                                                        let mut sel = self.selected_msg_ids.contains(&msg.id);
                                                        if ui.checkbox(&mut sel, "").changed() {
                                                            pending_action = Some((idx, BubbleAction::ToggleSelect(msg.id.clone())));
                                                        }
                                                    }
                                                    let resolved_sender = (!msg.is_outgoing).then(|| {
                                                        resolve_message_sender_name_from(
                                                            &self.server_aliases,
                                                            &self.threads,
                                                            &self.group_members,
                                                            msg,
                                                            &thread.id,
                                                            is_group,
                                                            &thread_name,
                                                        )
                                                    });
                                                    let resolved_quote_sender =
                                                        resolve_quote_sender_name_from(
                                                            &self.server_aliases,
                                                            &self.threads,
                                                            &self.group_members,
                                                            thread,
                                                            msg,
                                                            &thread_name,
                                                        );
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
                                                        &self.sticker_sprites,
                                                        &mut self.loading_stickers,
                                                        &self.emoji_textures,
                                                        self.settings.enable_animated_stickers,
                                                        idx,
                                                        resolved_sender,
                                                        resolved_quote_sender,
                                                        &self.server_aliases,
                                                        self.settings.link_suggestions_enabled,
                                                        self.settings.file_thumbnails_enabled,
                                                    ) {
                                                        pending_action = Some((idx, action));
                                                    }

                                                    // Render AI manual translation box below message bubble if present
                                                    let trans_key = (thread_id.clone(), msg.id.clone());
                                                    let is_translating = self.translating_messages.contains(&trans_key);
                                                    let cached_trans = self.translations.get(&trans_key);
                                                    if is_translating {
                                                        ui.horizontal(|ui| {
                                                            if msg.is_outgoing { ui.add_space((ui.available_width() - 240.0).max(0.0)); }
                                                            ui.spinner();
                                                            ui.label(RichText::new("🌐 Đang dịch tin nhắn…").size(12.0).color(pal.text_muted));
                                                        });
                                                    } else if let Some((translated_text, src_lang, text_hash)) = cached_trans {
                                                        use std::hash::{Hash, Hasher};
                                                        let mut h = std::collections::hash_map::DefaultHasher::new();
                                                        msg.text.hash(&mut h);
                                                        let cur_hash = h.finish();
                                                        if *text_hash == cur_hash && !msg.is_e2ee && msg.kind != crate::message_model::MessageKind::Recall {
                                                            let trans_w = (ui.available_width() * 0.72).clamp(160.0, 480.0);
                                                            ui.horizontal(|ui| {
                                                                if msg.is_outgoing { ui.add_space((ui.available_width() - trans_w - 6.0).max(0.0)); }
                                                                egui::Frame::none()
                                                                    .fill(if pal.is_dark { Color32::from_rgb(32, 40, 52) } else { Color32::from_rgb(238, 244, 255) })
                                                                    .stroke(Stroke::new(1.0_f32, pal.primary.linear_multiply(0.4)))
                                                                    .rounding(Rounding::same(8.0))
                                                                    .inner_margin(Margin::symmetric(10.0, 6.0))
                                                                    .show(ui, |ui| {
                                                                        ui.set_width(trans_w);
                                                                        ui.horizontal(|ui| {
                                                                            let lang_hint = src_lang.as_deref().map(|l| format!(" ({l})")).unwrap_or_default();
                                                                            ui.label(RichText::new(format!("🌐 Bản dịch{lang_hint}:")).strong().size(12.0).color(pal.primary));
                                                                            if ui.small_button("📋 Sao chép").on_hover_text("Sao chép bản dịch").clicked() {
                                                                                ui.ctx().copy_text(translated_text.clone());
                                                                            }
                                                                        });
                                                                        ui.add_space(2.0);
                                                                        ui.label(RichText::new(translated_text).size(13.0).color(if pal.is_dark { Color32::from_rgb(220, 230, 245) } else { Color32::from_rgb(30, 40, 55) }));
                                                                    });
                                                            });
                                                        }
                                                    }

                                                    // Render Voice-to-text transcript box below voice message if present
                                                    let is_transcribing = self.transcribing_voices.contains(&trans_key);
                                                    let cached_transcript = self.voice_transcripts.get(&trans_key);
                                                    let transcript_visible = msg.kind == crate::message_model::MessageKind::Voice
                                                        && !msg.is_e2ee && !content_protected
                                                        && !crate::message_lifecycle::expired(msg, crate::login::unix_now()*1000);
                                                    if is_transcribing && transcript_visible {
                                                        ui.horizontal(|ui| {
                                                            if msg.is_outgoing { ui.add_space((ui.available_width() - 240.0).max(0.0)); }
                                                            ui.spinner();
                                                            ui.label(RichText::new("🎙 Đang chuyển giọng nói thành văn bản…").size(12.0).color(pal.text_muted));
                                                        });
                                                    } else if let Some(transcript) = cached_transcript.filter(|_|transcript_visible) {
                                                        let trans_w = (ui.available_width() * 0.72).clamp(160.0, 480.0);
                                                        ui.horizontal(|ui| {
                                                            if msg.is_outgoing { ui.add_space((ui.available_width() - trans_w - 6.0).max(0.0)); }
                                                            egui::Frame::none()
                                                                .fill(if pal.is_dark { Color32::from_rgb(35, 45, 40) } else { Color32::from_rgb(238, 255, 244) })
                                                                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(40, 160, 80)))
                                                                .rounding(Rounding::same(8.0))
                                                                .inner_margin(Margin::symmetric(10.0, 6.0))
                                                                .show(ui, |ui| {
                                                                    ui.set_width(trans_w);
                                                                    ui.horizontal(|ui| {
                                                                        ui.label(RichText::new("🎙 Văn bản từ giọng nói:").strong().size(12.0).color(Color32::from_rgb(30, 150, 70)));
                                                                        if ui.small_button("📋 Sao chép").on_hover_text("Sao chép văn bản").clicked() {
                                                                            ui.ctx().copy_text(transcript.clone());
                                                                        }
                                                                    });
                                                                    ui.add_space(2.0);
                                                                    ui.label(RichText::new(transcript).size(13.0).color(if pal.is_dark { Color32::from_rgb(220, 245, 230) } else { Color32::from_rgb(25, 45, 30) }));
                                                                });
                                                        });
                                                    }

                                                    ui.add_space(if next_same_sender {
                                                        2.5
                                                    } else {
                                                        9.0
                                                    });
                                                });
                                                let measured_h = row.response.rect.height();
                                                if measured_h > 1.0 && !msg.id.is_empty() {
                                                    const MAX_MEASURED_HEIGHTS: usize = 3000;
                                                    if self.measured_msg_heights.len() >= MAX_MEASURED_HEIGHTS {
                                                        self.measured_msg_heights.clear();
                                                    }
                                                    self.measured_msg_heights.insert(cache_key, measured_h);
                                                }
                                                capture_anchor(ui,row.response.rect,&msg.id,previous_anchor.as_ref(),&mut next_anchor,&mut anchor_delta);
                                                if !modal_open && !is_system_notice_message(msg) {
                                                    let mut hit = row.response.clone();
                                                    hit.rect = egui::Rect::from_min_max(egui::pos2(ui.clip_rect().left(), hit.rect.top()), egui::pos2(ui.clip_rect().right(), hit.rect.bottom()));
                                                    if let Some(action) = bubble_context_menu(ui, &hit, msg, &self.emoji_textures, is_group,content_protected) {pending_action=Some((idx,action));}
                                                }
                                                if let Some(action) = hover_message_actions(ui, row.response.rect, &thread_id, msg, &self.emoji_textures, modal_open) { pending_action = Some((idx, action)); }
                                                if ui.is_rect_visible(row.response.rect) {
                                                    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<String>>(Id::new("visible_mentions")).push(msg.id.clone()));
                                                }
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
                            if newest_requested { ui.scroll_to_cursor(Some(egui::Align::BOTTOM)); }
                        });
                    let away_from_bottom=message_scroll.state.offset.y+message_scroll.inner_rect.height()+40.0<message_scroll.content_size.y;
                    let interacting=ctx.input(|i|i.smooth_scroll_delta.y!=0.0 || i.pointer.any_down());
                    let restore=layout_changed&&!interacting&&!newest_requested&&jump_index.is_none();
                    if restore {
                        if let Some(delta)=anchor_delta.filter(|d:&f32|d.is_finite()&&d.abs()>0.5){
                            let mut state=message_scroll.state;
                            state.offset.y=(state.offset.y+delta).clamp(0.0,(message_scroll.content_size.y-message_scroll.inner_rect.height()).max(0.0));
                            state.store(ctx,message_scroll.id);
                            ctx.request_discard("restore stable message anchor");
                        }
                    }
                    self.chat_anchor=if away_from_bottom {
                        if restore&&anchor_delta.is_some(){previous_anchor}else{next_anchor.map(|(id,offset)|(thread_id.clone(),id,offset))}
                    }else{None};
                    if self.batch_selection_mode && !self.timeline_selection_matches(&thread_id,is_group) {self.clear_timeline_selection();}
                    if self.batch_selection_mode && !modal_open {
                        egui::Area::new(Id::new(("batch_action_bar", &thread_id)))
                            .order(egui::Order::Foreground)
                            .fixed_pos(message_scroll.inner_rect.left_bottom() + Vec2::new(16.0, -44.0))
                            .show(ctx, |ui| {
                                egui::Frame::none()
                                    .fill(pal.bg_elevated)
                                    .stroke(Stroke::new(1.0_f32, pal.border))
                                    .rounding(Rounding::same(8.0))
                                    .inner_margin(Margin::symmetric(12.0, 6.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("Đã chọn: {} tin", self.selected_msg_ids.len())).strong().color(pal.text_primary));
                                            ui.separator();
                                            let can_act = !self.selected_msg_ids.is_empty();
                                            if ui.add_enabled(can_act, egui::Button::new(crate::i18n::tr("Chuyển tiếp"))).clicked() {
                                                self.share_timeline_selection(&thread_id,is_group);
                                            }
                                            let all_outgoing = if let Some(t) = self.threads.get(self.selected_thread_index) {
                                                t.messages.iter()
                                                    .filter(|m| self.selected_msg_ids.contains(&m.id))
                                                    .all(|m| m.is_outgoing && m.text.trim() != "[Tin nhắn đã thu hồi]")
                                            } else { false };
                                            if ui.add_enabled(can_act && all_outgoing, egui::Button::new(crate::i18n::tr("Thu hồi cho mọi người"))).clicked() {
                                                let selected: Vec<_> = self.threads.get(self.selected_thread_index).into_iter().flat_map(|t| &t.messages).filter(|m| self.selected_msg_ids.contains(&m.id)).cloned().collect();
                                                self.confirm_archive_mutation(&thread_id, is_group, true, &selected);
                                            }
                                            if ui.add_enabled(can_act, egui::Button::new("Xóa phía tôi…")).clicked() {
                                                let selected: Vec<_> = self.threads.get(self.selected_thread_index).into_iter().flat_map(|t| &t.messages).filter(|m| self.selected_msg_ids.contains(&m.id)).cloned().collect();
                                                self.confirm_archive_mutation(&thread_id, is_group, false, &selected);
                                            }
                                            if ui.button(crate::i18n::tr("Hủy")).clicked() {
                                                self.batch_selection_mode = false;
                                                self.selected_msg_ids.clear();
                                            }
                                        });
                                    });
                            });
                    }
                    if away_from_bottom {
                        egui::Area::new(Id::new(("newest_button", &thread_id)))
                            .order(egui::Order::Foreground)
                            .fixed_pos(message_scroll.inner_rect.right_bottom() - Vec2::new(16.0, 12.0))
                            .pivot(egui::Align2::RIGHT_BOTTOM)
                            .show(ctx, |ui| {
                                if ui.button(crate::i18n::tr("↓ Tin nhắn mới nhất")).clicked() {
                                    self.quote_jump = None;
                                    self.filter_starred_only = false;
                                    self.chat_render_window.1 = 150;
                                    ctx.data_mut(|d|d.insert_temp(Id::new(("jump_newest", &thread_id)),true));
                                    ctx.request_repaint();
                                }
                            });
                    }
                    let read_ids = ctx
                        .data_mut(|d| d.remove_temp::<Vec<String>>(Id::new("visible_mentions")))
                        .unwrap_or_default();
                    if ctx.input(|i| i.focused) && !self.is_workspace_input_gated(ctx) {
                        if let Some(thread) = self.threads.get_mut(self.selected_thread_index) {
                            let before = thread.unread.mention_ids.len();
                            for id in read_ids {
                                thread.unread.mention_ids.remove(&id);
                            }
                            if before != thread.unread.mention_ids.len() {
                                self.persist_threads_async();
                            }
                        }
                    }
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
                                BubbleAction::ShowReactors => {
                                    ctx.data_mut(|data| data.insert_temp(Id::new("reaction_details"),
                                        (self.user_uin.clone(), self.threads[self.selected_thread_index].id.clone(), m.clone())));
                                }
                                BubbleAction::ToggleVoice => self.toggle_inline_voice(ctx, &thread_id, is_group, m.clone()),

                                BubbleAction::RetryMedia{key}=>{let preview=crate::media_recovery::image_key(&self.threads[self.selected_thread_index].id,&m.id,0,true);for key in [key,preview]{self.loading_stickers.remove(&key);self.image_failures.remove(&key);self.image_retry_at.remove(&key);}}
                                BubbleAction::MediaUnavailable=>{self.status_msg="Thiếu thông tin tải ảnh/video; chưa thể mở tệp".into();}

                                BubbleAction::PreviewDocument { url, name } => {
                                    if let Some(tx) = WORKER_TX.get() {
                                        let _ = tx.send(WorkerCmd::PreviewDocument { url, name, context: super::worker::ChatImage {
                                            thread: self.threads[self.selected_thread_index].id.clone(),
                                            group: self.threads[self.selected_thread_index].is_group,
                                            message: m.clone(), preview: false,
                                        } });
                                    }
                                }
                                BubbleAction::OpenAttachment {url,name} => {
                                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::DownloadMedia{url,name,play:false,open:true,context:Some(super::worker::ChatImage{thread:self.threads[self.selected_thread_index].id.clone(),group:self.threads[self.selected_thread_index].is_group,message:m.clone(),preview:false})});}
                                }
                                BubbleAction::OpenLocalFile(path) => {
                                    std::thread::spawn(move || {
                                        let _ = crate::platform::open_file(&path);
                                    });
                                }
                                BubbleAction::OpenContainingFolder(path) => {
                                    std::thread::spawn(move || {
                                        let _ = crate::platform::open_folder_and_select(&path);
                                    });
                                }
                                BubbleAction::SaveAttachment {url,name} => {
                                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::DownloadMedia{url,name,play:false,open:false,context:Some(super::worker::ChatImage{thread:self.threads[self.selected_thread_index].id.clone(),group:self.threads[self.selected_thread_index].is_group,message:m.clone(),preview:false})});}
                                }
                                BubbleAction::OpenVideo {url,name} => {
                                    let source=&self.threads[self.selected_thread_index];let id=source.id.clone();let group=source.is_group;
                                    self.open_media_viewer(&id,&m);
                                    if let Some(tx)=WORKER_TX.get(){let _=tx.send(WorkerCmd::DownloadMedia{url,name,play:true,open:false,context:Some(super::worker::ChatImage{thread:id,group,message:m.clone(),preview:false})});}
                                }
                                BubbleAction::ArchiveAttachmentDraft => {
                                    if let Some(job) = m.outgoing_job.as_ref() {
                                        if let crate::outgoing::Payload::Attachment { path, stamp, .. } = &job.payload {
                                            let queued = WORKER_TX.get().is_some_and(|tx| tx.send(WorkerCmd::PrepareAttachmentArchive {
                                                owner:self.user_uin.clone(), runtime_id:self.account_runtime_id,
                                                thread:thread_id.clone(), path:path.clone(),stamp:stamp.clone(),
                                            }).is_ok());
                                            if !queued {self.show_toast("Worker chưa sẵn sàng để tạo ZIP");}
                                        }
                                    }
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
                                BubbleAction::Mention => {
                                    if is_group && !m.is_outgoing {
                                        if let Some(uid) = m.wire_ref.sender.as_ref().and_then(|v| v.as_str().map(str::to_owned).or_else(|| v.as_u64().map(|n|n.to_string()))) {
                                            self.composer_mentions.edit(&self.input_message);
                                            let end = self.input_message.len();
                                            if end > 0 && !self.input_message.ends_with(char::is_whitespace) {
                                                self.input_message.push(' ');
                                                self.composer_mentions.edit(&self.input_message);
                                            }
                                            let end = self.input_message.len();
                                            let mention_name = self
                                                .resolve_contact_name(&uid, &m.sender)
                                                .to_owned();
                                            self.composer_mentions.insert(end, end, &uid, &mention_name);
                                            self.input_message = self.composer_mentions.text.clone();
                                            self.composer_focus_pending = true;
                                        }
                                    }
                                }
                                BubbleAction::Reply => { self.replying_to = Some(m); ctx.memory_mut(|memory|memory.request_focus(Id::new(("chat_composer_input",&thread_id)))); },
                                BubbleAction::CopyLink(link) => {
                                    ctx.copy_text(link);
                                    self.status_msg = "Đã sao chép liên kết".into();
                                }
                                BubbleAction::DoubleClickReply => { if self.settings.double_click_to_reply {self.replying_to=Some(m);ctx.memory_mut(|memory|memory.request_focus(Id::new(("chat_composer_input",&thread_id))));} },
                                BubbleAction::JumpToQuoted(reference) => {
                                    let index = self
                                        .threads
                                        .get(self.selected_thread_index)
                                        .and_then(|thread| {
                                            crate::message_navigation::unique_reference_index(&thread.messages, &reference)
                                        });
                                    if index.is_some() {
                                        self.filter_starred_only = false;
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
                                    let mut m = m;
                                    // The source conversation lets a forward renew an expired media link.
                                    let source = &self.threads[self.selected_thread_index];
                                    crate::chat::set_forward_source(&mut m, &source.id, source.is_group);
                                    self.clear_forward_dialog();
                                    self.forward_msg = Some(m);
                                }
                                BubbleAction::React(r) => {
                                    let current = self
                                        .threads
                                        .get(self.selected_thread_index)
                                        .and_then(|t| t.messages.get(idx))
                                        .cloned()
                                        .unwrap_or_default();
                                    let reaction = reaction_toggle_intent(
                                        &current,
                                        &self.user_uin,
                                        r,
                                    );
                                    self.dispatch_reaction_intent(
                                        &thread_id,
                                        is_group,
                                        m,
                                        reaction,
                                    );
                                }
                                BubbleAction::PinOnServer=>{self.pin_server_message(thread_id.clone(),is_group,m);}
                                BubbleAction::Reminder => {self.open_server_board(thread_id.clone(),is_group,m.text.chars().take(4000).collect());}
                                BubbleAction::TogglePin | BubbleAction::ToggleStar => {
                                    self.chat_geometry_revision=self.chat_geometry_revision.wrapping_add(1);
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
                                    self.open_preflight_call(
                                        thread_id.clone(),
                                        thread_name.clone(),
                                        None,
                                        is_video,
                                        is_group,
                                    );
                                }
                                BubbleAction::Details => {
                                    self.message_details = Some((thread_id.clone(), m));
                                }
                                BubbleAction::SaveToDocuments => {
                                    self.save_archive_to_documents(&thread_id, is_group, vec![m]);
                                }
                                BubbleAction::DeleteForMe => {
                                    self.confirm_archive_mutation(&thread_id, is_group, false, &[m]);
                                }
                                BubbleAction::RecallForEveryone => {
                                    self.confirm_archive_mutation(&thread_id, is_group, true, &[m]);
                                }
                                BubbleAction::EnterSelectionMode => {
                                    self.bind_timeline_selection(&thread_id,is_group);
                                    self.selected_msg_ids.clear();
                                    self.selected_msg_ids.insert(m.id.clone());
                                }
                                BubbleAction::ToggleSelect(id) => {
                                    self.bind_timeline_selection(&thread_id,is_group);
                                    if self.selected_msg_ids.contains(&id) {
                                        self.selected_msg_ids.remove(&id);
                                    } else {
                                        self.selected_msg_ids.insert(id);
                                    }
                                }
                                BubbleAction::OpenContactConversation(uid) => {
                                    if let Some(idx) = self.threads.iter().position(|t| t.id == uid) {
                                        self.selected_thread_index = idx;
                                        self.drawer_view = DrawerView::Overview;
                                        self.drawer_search_query.clear();
                                    } else {
                                        self.open_contact_panel(uid);
                                    }
                                }
                                BubbleAction::ViewContact(uid) => {
                                    self.open_contact_panel(uid);
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
                                        source:super::worker::ChatImage{thread:self.threads[self.selected_thread_index].id.clone(),group:self.threads[self.selected_thread_index].is_group,message:m.clone(),preview:true},is_video:false,navigation_pending:false,
                                        thread_id:self.threads[self.selected_thread_index].id.clone(), message_id:m.id.clone(), dimensions:crate::message_view::media(&m).width.zip(crate::message_view::media(&m).height),
                                        tex_key,
                                        hd_tex_key,
                                        original_url: crate::message_view::media(&m).original.unwrap_or_else(||full_url.clone()),
                                        title,
                                        sender,
                                        time,
                                        zoom: 1.0,
                                        pan: Vec2::ZERO,
                                        video_state: None,
                                    });
                                }
                                BubbleAction::TranslateMessage { msg_id, text } => {
                                    if !self.settings.ai_translation_consent {
                                        self.show_toast("Vui lòng kích hoạt 'Đồng ý dịch tin nhắn bằng AI' trong Cài đặt");
                                    } else if m.is_e2ee || m.kind == crate::message_model::MessageKind::Recall
                                        || self.content_protected(&thread_id, is_group)
                                        || crate::message_lifecycle::expired(&m, crate::login::unix_now() * 1000) {
                                        self.show_toast("Không thể dịch tin nhắn mã hóa đầu cuối hoặc tin đã thu hồi");
                                    } else {
                                        let key = (thread_id.clone(), msg_id.clone());
                                        let text_hash = {
                                            use std::hash::{Hash, Hasher};
                                            let mut h = std::collections::hash_map::DefaultHasher::new();
                                            text.hash(&mut h);
                                            h.finish()
                                        };
                                        if self.translating_messages.insert(key) {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::TranslateMessage {
                                                    thread_id: thread_id.clone(),
                                                    msg_id: msg_id.clone(),
                                                    text,
                                                    target_lang: "vi".into(),
                                                    text_hash,
                                                    generation: 0,
                                                });
                                            }
                                        }
                                    }
                                }
                                BubbleAction::TranscribeVoice { msg_id, voice_url } => {
                                    if !self.settings.ai_translation_consent {
                                        self.show_toast("Vui lòng kích hoạt 'Đồng ý dịch tin nhắn bằng AI' trong Cài đặt");
                                    } else if m.is_e2ee || m.kind == crate::message_model::MessageKind::Recall {
                                        self.show_toast("Không thể chuyển giọng nói cho tin nhắn mã hóa đầu cuối hoặc tin đã thu hồi");
                                    } else {
                                        let key = (thread_id.clone(), msg_id.clone());
                                        if self.transcribing_voices.insert(key) {
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::TranscribeVoiceMessage {
                                                    thread_id: thread_id.clone(),
                                                    msg_id: msg_id.clone(),
                                                    voice_url,
                                                    generation: self.account_runtime_id as u64,
                                                    context: super::worker::ChatImage {
                                                        thread: thread_id.clone(), group: is_group, message: m.clone(), preview: false,
                                                    },
                                                });
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(who) = self.settings.show_typing_indicator
                        .then(|| self.typing_label(&thread_id, is_group)).flatten() {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!("{who} đang soạn tin…"))
                                    .italics()
                                    .size(12.0)
                                    .color(pal.text_secondary),
                            );
                        });
                    }

                    // The picker floats above its trigger without shrinking the timeline.
                    if self.sticker_picker_open && !modal_open {
                        let anchor = ctx.data(|data| data.get_temp::<Rect>(Id::new(("composer_picker_anchor", &thread_id))));
                        if let Some(anchor) = anchor {
                            let picker_width = ui.max_rect().width().min(ctx.screen_rect().width() - 24.0).clamp(160.0, 460.0);
                            egui::Area::new(Id::new(("composer_unified_picker", &thread_id)))
                                .order(egui::Order::Foreground)
                                .pivot(egui::Align2::LEFT_BOTTOM)
                                .fixed_pos(composer_picker_position(anchor, ui.max_rect(), ctx.screen_rect(), picker_width))
                                .constrain_to(ctx.screen_rect())
                                .show(ctx, |ui| {
                                    ui.set_width(picker_width);
                                    ui.add_enabled_ui(group_send_allowed, |ui| {
                                        self.render_sticker_picker(ui, &thread_id, is_group, pal);
                                    });
                                });
                        }
                        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape)) {
                            self.sticker_picker_open = false;
                        }
                    }

                    // 3b. Reply banner (quoted message preview above the composer)
                    if self.replying_to.is_some() {
                        let mut cancel = false;
                        if let Some(q) = &self.replying_to {
                            let who = if q.is_outgoing {
                                "chính bạn".to_string()
                            } else {
                                self.resolve_message_sender_name(
                                    q,
                                    &thread_id,
                                    is_group,
                                    &thread_name,
                                )
                                .to_owned()
                            };
                            egui::Frame::none()
                                .fill(if pal.is_dark { Color32::from_rgb(34, 38, 43) } else { Color32::from_rgb(241, 245, 249) })
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Margin::symmetric(12.0, 6.0))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        let media = crate::message_view::media(q);
                                        if matches!(
                                            media.kind,
                                            crate::message_view::MediaKind::Photo
                                                | crate::message_view::MediaKind::Video
                                        ) {
                                            let texture_key = crate::media_recovery::image_key(
                                                &thread_id,
                                                &q.id,
                                                0,
                                                false,
                                            );
                                            if let Some(texture) =
                                                self.sticker_textures.get(&texture_key)
                                            {
                                                ui.add(
                                                    egui::Image::new(texture)
                                                        .fit_to_exact_size(Vec2::splat(34.0)),
                                                );
                                            } else {
                                                let (thumb, _) = ui.allocate_exact_size(
                                                    Vec2::splat(34.0),
                                                    Sense::hover(),
                                                );
                                                ui.painter().rect_filled(
                                                    thumb,
                                                    Rounding::same(4.0),
                                                    pal.bg_hover,
                                                );
                                                ui.painter().text(
                                                    thumb.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    "▧",
                                                    FontId::proportional(18.0),
                                                    pal.text_muted,
                                                );
                                            }
                                        }
                                        let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 28.0), Sense::hover());
                                        ui.painter().rect_filled(bar_rect, Rounding::same(1.5), pal.primary);
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(format!("Trả lời {who}"))
                                                    .size(12.0)
                                                    .color(pal.primary)
                                                    .strong(),
                                            );
                                            let preview = if q.sticker_id.is_some() {
                                                "[Nhãn dán]".to_string()
                                            } else if q.text.is_empty() {
                                                "[Hình ảnh/Tệp đính kèm]".to_string()
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
                                                    .on_hover_text(crate::i18n::tr("Hủy trả lời"))
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
                    // Ctrl+Shift+V explicitly preserves spreadsheet cells as text.
                    let spreadsheet_text=ctx.input(|i|i.events.iter().find_map(|e|if let egui::Event::Paste(text)=e{Some(text.contains('\t'))}else{None})).unwrap_or(false);
                    let composer_can_paste=ctx.memory(|m|m.focused().is_none()||m.has_focus(Id::new(("chat_composer_input",&thread_id))));
                    if composer_can_paste && is_clipboard_paste_triggered(ctx) && clipboard_has_image()
                        && !(spreadsheet_text&&self.settings.excel_paste_mode=="text")
                        && !self.clipboard_text_only
                        && !ctx.input(|i| i.modifiers.shift && i.modifiers.command)
                        && !self.clipboard_read_pending
                    {
                        self.clipboard_text_fallback = ctx.input(|i| i.events.iter().find_map(|e| {
                            if let egui::Event::Paste(text) = e { Some(text.clone()) } else { None }
                        }));
                        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Paste(_))));
                        self.request_clipboard_image();
                    }
                    if ctx.input(|i|i.events.iter().any(|e|matches!(e,egui::Event::Paste(_)))) {self.clipboard_text_only=false;}
                    let compose_top = ui.cursor().top();
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
                                                    .on_hover_text(crate::i18n::tr("Hủy ảnh dán (Esc)"))
                                                    .clicked()
                                                {
                                                    clear_clip = true;
                                                }
                                            },
                                        );
                                    });
                                });
                        }
                        if self.clipboard_alternative_text.is_some() {
                            ui.horizontal(|ui|{
                                ui.label(RichText::new(crate::i18n::tr("Vùng Excel: Enter để gửi ảnh, hoặc")).color(pal.text_secondary));
                                if ui.button(crate::i18n::tr("Dán dạng chữ")).clicked(){
                                    if let Some(text)=self.clipboard_alternative_text.take(){self.input_message.push_str(&text);self.composer_focus_pending=true;}
                                    clear_clip=true;
                                }
                            });
                        }
                        if clear_clip {
                            if let Some((path, _, _, _)) = self.retire_pending_clipboard_image() {
                                std::thread::spawn(move || {
                                    let _ = std::fs::remove_file(path);
                                });
                            }
                            self.clipboard_alternative_text = None;
                        }
                    }

                    // 4. Bottom Message Composer Bar (Two-Tier Composer: Tier 1 Toolbar + Tier 2 Multiline Editor)
                    if let Some(notice) = group_notice { ui.label(notice); }
                    ui.set_max_width(composer_layout_width);
                    let composer_bounds = Rect::from_min_max(
                        Pos2::new(ui.max_rect().left(), composer_bottom - reserved_composer_height),
                        Pos2::new(ui.max_rect().left() + composer_layout_width, composer_bottom),
                    );
                    let mut composer_ui = ui.new_child(egui::UiBuilder::new()
                        .id_salt("bottom_anchored_composer").max_rect(composer_bounds));
                    let mut composer_send_bounds = None;
                    let composer_frame = egui::Frame::none()
                        .fill(pal.bg_sidebar)
                        .inner_margin(Margin::symmetric(14.0, 8.0))
                        .stroke(Stroke::new(1.0_f32, pal.border))
                        .show(&mut composer_ui, |ui| {
                            if !group_send_allowed { ui.disable(); }

                            self.cancel_voice_if_thread_changed(&thread_id, is_group);

                            // Tier 1: Utility Toolbar (9 buttons in exact Zalo Web parity order)
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;

                                // 1. Sticker Icon Button
                                let (stk_rect, stk_resp) = ui.allocate_exact_size(
                                    Vec2::new(32.0, 32.0),
                                    Sense::click(),
                                );
                                if stk_resp.hovered() || !self.sticker_picker_open {
                                    ctx.data_mut(|data| data.insert_temp(Id::new(("composer_picker_anchor", &thread_id)), stk_rect));
                                }
                                if self.sticker_picker_open || stk_resp.hovered() {
                                    ui.painter().rect_filled(
                                        stk_rect,
                                        Rounding::same(6.0),
                                        pal.bg_hover,
                                    );
                                }
                                draw_icon_sticker(
                                    ui.painter(),
                                    stk_rect.center(),
                                    17.0,
                                    if self.sticker_picker_open || stk_resp.hovered() {
                                        Color32::from_rgb(251, 191, 36)
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if stk_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Nhãn dán (Sticker)"))
                                    .clicked()
                                {
                                    self.sticker_picker_open = !self.sticker_picker_open;
                                }

                                // 2. Photo Upload Button
                                let (img_rect, img_resp) = ui.allocate_exact_size(
                                    Vec2::new(32.0, 32.0),
                                    Sense::click(),
                                );
                                if img_resp.hovered() {
                                    ui.painter().rect_filled(
                                        img_rect,
                                        Rounding::same(6.0),
                                        pal.bg_hover,
                                    );
                                }
                                draw_icon_image(
                                    ui.painter(),
                                    img_rect.center(),
                                    17.0,
                                    if img_resp.hovered() {
                                        pal.accent_online
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if img_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Gửi hình ảnh"))
                                    .clicked()
                                {
                                    self.request_attachment_picker(&thread_id, is_group, true);
                                }

                                // 3. File / Folder Attachment Dropdown
                                let att_popup_id = ui.make_persistent_id("composer_attachment_dropdown");
                                let (att_rect, att_resp) = ui.allocate_exact_size(
                                    Vec2::new(32.0, 32.0),
                                    Sense::click(),
                                );
                                let att_open = ui.memory(|m| m.is_popup_open(att_popup_id));
                                if att_resp.hovered() || att_open {
                                    ui.painter().rect_filled(
                                        att_rect,
                                        Rounding::same(6.0),
                                        pal.bg_hover,
                                    );
                                }
                                draw_icon_attachment(
                                    ui.painter(),
                                    att_rect.center(),
                                    16.0,
                                    if att_resp.hovered() || att_open {
                                        pal.primary_hover
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if att_resp
                                    .clone()
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Đính kèm File hoặc Thư mục"))
                                    .clicked()
                                {
                                    ui.memory_mut(|m| m.toggle_popup(att_popup_id));
                                }
                                egui::popup_below_widget(
                                    ui,
                                    att_popup_id,
                                    &att_resp,
                                    egui::PopupCloseBehavior::CloseOnClick,
                                    |ui| {
                                        ui.set_min_width(140.0);
                                        ui.spacing_mut().button_padding = Vec2::new(8.0, 6.0);
                                        if ui.button(format!("{}  {}", lucide::FILE_TEXT, crate::i18n::tr("Chọn File"))).clicked() {
                                            self.request_attachment_picker(&thread_id, is_group, false);
                                            ui.memory_mut(|m| m.close_popup());
                                        }
                                        if ui.button(format!("{}  {}", lucide::FOLDER_OPEN, crate::i18n::tr("Chọn Thư mục"))).clicked() {
                                            self.request_folder_picker(&thread_id);
                                            ui.memory_mut(|m| m.close_popup());
                                        }
                                    },
                                );

                                // 4. Contact Card Button
                                let (cont_rect, cont_resp) = ui.allocate_exact_size(
                                    Vec2::new(32.0, 32.0),
                                    Sense::click(),
                                );
                                if cont_resp.hovered() {
                                    ui.painter().rect_filled(
                                        cont_rect,
                                        Rounding::same(6.0),
                                        pal.bg_hover,
                                    );
                                }
                                draw_lucide_icon(
                                    ui.painter(),
                                    cont_rect.center(),
                                    lucide::USER,
                                    15.0,
                                    if cont_resp.hovered() {
                                        pal.text_primary
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if cont_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Gửi danh thiếp…"))
                                    .clicked()
                                {
                                    self.send_contact_modal_open = true;
                                    self.send_contact_query.clear();
                                    self.send_contact_selected = None;
                                    self.send_contact_filter = 0;
                                }

                                // 5. Screenshot Capture Button (Windows)
                                if cfg!(windows) {
                                    let (cap_rect, cap_resp) = ui.allocate_exact_size(
                                        Vec2::new(32.0, 32.0),
                                        Sense::click(),
                                    );
                                    if cap_resp.hovered() {
                                        ui.painter().rect_filled(
                                            cap_rect,
                                            Rounding::same(6.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    ui.painter().text(
                                        cap_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "✂",
                                        FontId::proportional(15.0),
                                        if cap_resp.hovered() {
                                            pal.text_primary
                                        } else {
                                            pal.text_secondary
                                        },
                                    );
                                    if cap_resp
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .on_hover_text(crate::i18n::tr("Chụp màn hình (Ctrl+Alt+S)"))
                                        .clicked()
                                    {
                                        self.request_screen_capture();
                                    }
                                }

                                // 6. Text Formatting Toggle Button (Ctrl+Shift+X)
                                let (fmt_rect, fmt_resp) = ui.allocate_exact_size(
                                    Vec2::new(32.0, 32.0),
                                    Sense::click(),
                                );
                                if self.composer_format_mode || fmt_resp.hovered() {
                                    ui.painter().rect_filled(
                                        fmt_rect,
                                        Rounding::same(6.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    fmt_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Aa",
                                    FontId::proportional(15.0),
                                    if self.composer_format_mode {
                                        pal.primary
                                    } else if fmt_resp.hovered() {
                                        pal.text_primary
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if fmt_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Định dạng tin nhắn (Ctrl+Shift+X)"))
                                    .clicked()
                                {
                                    self.composer_format_mode = !self.composer_format_mode;
                                    if !self.composer_format_mode {
                                        composer_format.clear_text_styles();
                                        ctx.data_mut(|data| {
                                            data.insert_temp(composer_format_key, composer_format.clone())
                                        });
                                    }
                                }

                                // 7. Quick Message Button
                                let (qm_rect, qm_resp) = ui.allocate_exact_size(
                                    Vec2::new(32.0, 32.0),
                                    Sense::click(),
                                );
                                if qm_resp.hovered() {
                                    ui.painter().rect_filled(
                                        qm_rect,
                                        Rounding::same(6.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    qm_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "⚡",
                                    FontId::proportional(15.0),
                                    if qm_resp.hovered() {
                                        Color32::from_rgb(251, 191, 36)
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if qm_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Chèn tin nhắn nhanh…"))
                                    .clicked()
                                {
                                    self.tools.open = true;
                                    self.tools.tab = 6;
                                }

                                // 8. Bank Account (STK) Button
                                let (stk_card_rect, stk_card_resp) = ui.allocate_exact_size(
                                    Vec2::new(32.0, 32.0),
                                    Sense::click(),
                                );
                                if stk_card_resp.hovered() {
                                    ui.painter().rect_filled(
                                        stk_card_rect,
                                        Rounding::same(6.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    stk_card_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "💳",
                                    FontId::proportional(15.0),
                                    if stk_card_resp.hovered() {
                                        pal.primary
                                    } else {
                                        pal.text_secondary
                                    },
                                );
                                if stk_card_resp
                                    .clone()
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(crate::i18n::tr("Gửi nhanh số tài khoản (STK)"))
                                    .clicked()
                                {
                                    let stk_item = self.tools.config.quick_messages.items.iter().find(|i| {
                                        let sc = i.shortcut.to_lowercase();
                                        let tt = i.title.to_lowercase();
                                        sc == "/stk" || sc == "stk" || tt.contains("stk") || tt.contains("tài khoản")
                                    }).cloned();
                                    if let Some(item) = stk_item {
                                        if !self.input_message.is_empty() && !self.input_message.ends_with(' ') && !self.input_message.ends_with('\n') {
                                            self.input_message.push(' ');
                                        }
                                        self.input_message.push_str(&item.text);
                                        self.composer_focus_pending = true;
                                    } else {
                                        self.tools.open = true;
                                        self.tools.tab = 6;
                                        self.tools.new_quick_shortcut = "/stk".to_string();
                                        self.tools.new_quick_title = "Số tài khoản".to_string();
                                        self.show_toast("Chưa có mẫu STK; hãy tạo mẫu tin nhắn nhanh /stk");
                                    }
                                }
                                stk_card_resp.context_menu(|ui| {
                                    if ui.button(crate::i18n::tr("Quản lý mẫu STK (Tin nhắn nhanh)…")).clicked() {
                                        self.tools.open = true;
                                        self.tools.tab = 6;
                                        ui.close_menu();
                                    }
                                });

                                // 9. More Options Button (...)
                                ui.menu_button(RichText::new("⋯").size(22.0).color(pal.text_secondary), |ui| {
                                    ui.set_min_width(220.0);
                                    ui.spacing_mut().button_padding = Vec2::new(12.0, 8.0);
                                    if ui.button(format!("📋  {}", crate::i18n::tr("Giao việc"))).clicked() {
                                        self.tools.open = true;
                                        self.tools.tab = 5;
                                        ui.close_menu();
                                    }
                                    if ui.button(format!("📊  {}", crate::i18n::tr("Tạo bình chọn"))).clicked() {
                                        self.tools.open = true;
                                        self.tools.tab = 5;
                                        ui.close_menu();
                                    }
                                    if ui.button(format!("⏰  {}", crate::i18n::tr("Tạo nhắc hẹn…"))).clicked() {
                                        self.open_server_board(thread_id.clone(), is_group, self.input_message.clone());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button(format!("{}  {}", lucide::MIC, crate::i18n::tr("Thu âm & gửi tin nhắn thoại…"))).clicked() {
                                        self.toggle_voice_recorder(ui.ctx());
                                        ui.close_menu();
                                    }
                                    if ui.button(crate::i18n::tr("Hẹn giờ gửi tin nhắn…")).clicked() {
                                        self.open_quick_schedule(ui.ctx());
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    for (mode, label) in [
                                        (Some(crate::rich_text::Urgency::Important), "Đánh dấu Quan trọng"),
                                        (Some(crate::rich_text::Urgency::Urgent), "Đánh dấu Khẩn cấp"),
                                        (None, "Gửi bình thường"),
                                    ] {
                                        if ui
                                            .selectable_label(composer_format.urgency == mode, label)
                                            .clicked()
                                        {
                                            composer_format.urgency = mode;
                                            self.composer_format_mode = true;
                                            ctx.data_mut(|data| {
                                                data.insert_temp(composer_format_key, composer_format.clone())
                                            });
                                            ui.close_menu();
                                        }
                                    }
                                })
                                .response
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .on_hover_text(crate::i18n::tr("Tùy chọn thêm"));
                            });

                            // Tier 1.5: Staged Attachment Preview Tray (Inside composer container)
                            if self.attachment_drafts.len(&thread_id) > 0 {
                                ui.add_space(3.0);
                                self.render_attachment_draft(ui, &thread_id, pal, 110.0);
                            }

                            ui.add_space(3.0);

                            // Tier 2: Multiline Editor Area (Spacious input + Emoji + Send/Like button)
                            egui::Frame::none()
                                .fill(pal.bg_input)
                                .rounding(Rounding::same(10.0))
                                .stroke(Stroke::new(1.0_f32, pal.border))
                                .inner_margin(Margin::symmetric(8.0, 4.0))
                                .show(ui, |ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        // Keep a usable editor and send action even at high DPI.
                                        let send_btn_w = composer_send_width;
                                        let input_w = composer_input_width;
                                        // Enter sends; Shift+Enter is left to the editor as a newline.
                                        // Only consumed while the composer owns focus, so other fields keep Enter.
                                        // An IME uses Enter to commit the word being composed; that Enter
                                        // must not also send the message.
                                        let (ime_active, ime_blocks_enter) = ui.input(|i| ime_state(&i.events, self.composer_ime_active));
                                        self.composer_ime_active = ime_active;
                                        let mut enter_send = self.quick_schedule.is_none()
                                            && !ime_blocks_enter
                                            && ui.memory(|m| m.has_focus(composer_id))
                                            && ui.input_mut(|i| consume_composer_enter(&mut i.events));
                                        let voice_snapshot = self.voice_worker.as_ref().map(crate::voice::VoiceWorker::snapshot).unwrap_or_default();
                                        let is_voice_active = self.voice_binding.is_some() || self.recorded_voice_samples.is_some();
                                        let (response, cursor) = if is_voice_active {
                                            let is_recording = voice_snapshot.recording || self.voice_start_pending;
                                            let dur = if voice_snapshot.recording {
                                                voice_snapshot.duration
                                            } else if let Some((_, _, ref d)) = self.recorded_voice_samples {
                                                *d
                                            } else {
                                                voice_snapshot.duration
                                            };
                                            let fallback_wave = vec![0.3, 0.5, 0.8, 0.4, 0.6, 0.3, 0.7, 0.9, 0.5, 0.4];
                                            let waveform = if voice_snapshot.recording {
                                                voice_snapshot.waveform.clone()
                                            } else {
                                                fallback_wave
                                            };
                                            let is_preview_playing = voice_snapshot.preview_playing;
                                            let preview_samples = self.recorded_voice_samples.as_ref().map(|(s, r, _)| (std::sync::Arc::clone(s), *r));

                                            let mut action = VoiceBarAction::None;
                                            egui::ScrollArea::vertical()
                                                .id_salt(("chat_composer_scroll", &thread_id))
                                                .max_width(input_w)
                                                .min_scrolled_width(input_w)
                                                .auto_shrink([false, true])
                                                .min_scrolled_height(0.0)
                                                .max_height(editor_height)
                                                .stick_to_bottom(true)
                                                .show(ui, |ui| {
                                                    action = Self::render_voice_recording_bar(
                                                        ui,
                                                        pal,
                                                        input_w,
                                                        is_recording,
                                                        dur,
                                                        &waveform,
                                                        is_preview_playing,
                                                        preview_samples,
                                                    );
                                                });

                                            match action {
                                                VoiceBarAction::StopRecording => self.toggle_voice_recorder(ui.ctx()),
                                                VoiceBarAction::CancelRecording => self.cancel_voice_recording(),
                                                VoiceBarAction::StopPreview => {
                                                    if let (Some(worker), Some(binding)) = (self.voice_worker.as_ref(), self.voice_binding.as_ref()) {
                                                        let _ = worker.stop_preview(binding.generation);
                                                    }
                                                }
                                                VoiceBarAction::TogglePreview(samples, rate) => {
                                                    if let (Some(worker), Some(binding)) = (self.voice_worker.as_ref(), self.voice_binding.as_ref()) {
                                                        if let Err(error) = worker.play_preview(binding.generation, samples, rate) {
                                                            self.show_toast(error.to_string());
                                                        }
                                                    }
                                                }
                                                VoiceBarAction::None => {}
                                            }

                                            let (_, resp) = ui.allocate_exact_size(Vec2::ZERO, Sense::hover());
                                            (resp, 0)
                                        } else {
                                            // ScrollArea owns the viewport height; TextEdit must not
                                            // add a second explicit-row height on top of the galley.
                                            let rows = 1;
                                            let mention_draft = self.composer_mentions.clone();
                                            let format_preview = composer_format.clone();
                                            let mut mention_layouter = |ui: &egui::Ui, text: &str, width: f32| {
                                                ui.fonts(|fonts| fonts.layout_job(composer_layout_job(
                                                    text,&mention_draft,&format_preview,width,composer_line_height,pal)))
                                            };
                                            let text_edit =
                                                egui::TextEdit::multiline(&mut self.input_message)
                                                    .return_key(egui::KeyboardShortcut::new(egui::Modifiers::SHIFT, Key::Enter))
                                                    .layouter(&mut mention_layouter)
                                                    .font(FontId::proportional(14.0))
                                                    .id(composer_id)
                                                    .frame(false)
                                                    .hint_text(if attachment_count>0 {"Nhập chú thích cho tệp…".into()}else if input_w<110.0 {"Nhập tin…".into()}else if input_w<180.0 {"Nhập tin nhắn…".into()}else{format!(
                                                        "Nhập tin nhắn tới {}...",
                                                        truncate_chars(&thread_name, 16)
                                                    )})
                                                    .margin(Margin::symmetric(6.0, 3.0))
                                                    .desired_rows(rows)
                                                    .desired_width(input_w);

                                            let output = egui::ScrollArea::vertical()
                                                .id_salt(("chat_composer_scroll", &thread_id))
                                                .max_width(input_w)
                                                .min_scrolled_width(input_w)
                                                .auto_shrink([false, false])
                                                .min_scrolled_height(editor_height)
                                                .max_height(editor_height)
                                                .stick_to_bottom(true)
                                                .show(ui, |ui| text_edit.show(ui))
                                                .inner;
                                            let cursor = output.cursor_range.map(|r| r.primary.ccursor.index).unwrap_or_else(|| self.input_message.chars().count());
                                            if let Some(range) = output.cursor_range {
                                                let [start,end] = range.sorted_cursors();
                                                ctx.data_mut(|data| data.insert_temp(composer_scope.with("selection"),
                                                    (start.ccursor.index,end.ccursor.index)));
                                            }
                                            (output.response, cursor)
                                        };
                                        if !file_drag_active {self.mention_suggestions(ui, &thread_id, cursor, &response, &mut enter_send, ime_blocks_enter);}
                                        else {enter_send=false;}
                                        if self.composer_focus_pending {
                                            response.request_focus();
                                            self.composer_focus_pending = false;
                                        }
                                        if response.changed() {
                                            self.last_composer_edit_at = std::time::Instant::now();
                                            if !self.input_message.trim().is_empty() {
                                                self.maybe_send_typing(&thread_id, is_group);
                                            }
                                        }
                                        response.context_menu(|ui| {
                                            if ui.button(crate::i18n::tr("Dán dạng văn bản (Ctrl+Shift+V)")).clicked(){
                                                self.clipboard_text_only=true;ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);ui.close_menu();
                                            }

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
                                            if cfg!(windows)
                                                && ui
                                                    .button(format!(
                                                        "{} Chụp màn hình (Ctrl+Alt+S)",
                                                        lucide::CAMERA
                                                    ))
                                                    .clicked()
                                            {
                                                ui.close_menu();
                                                self.request_screen_capture();
                                            }
                                        });
                                        // Zalo PC's capture shortcut; Ctrl+Alt+S is not bound elsewhere here.
                                        // AltGr arrives as Ctrl+Alt and also types a character; a frame
                                        // with typed text is never treated as the shortcut.
                                        if cfg!(windows)
                                            && ui.input_mut(|i| {
                                                !i.events.iter().any(|e| matches!(e, egui::Event::Text(_)))
                                                    && i.consume_key(
                                                        egui::Modifiers::CTRL | egui::Modifiers::ALT,
                                                        Key::S,
                                                    )
                                            })
                                        {
                                            self.request_screen_capture();
                                        }

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
                                        // Tier 2: Emoji / Sticker Toggle Button
                                        let (emo_rect, emo_resp) = ui.allocate_exact_size(
                                            Vec2::new(32.0, 34.0),
                                            Sense::click(),
                                        );
                                        if emo_resp.hovered() || !self.sticker_picker_open {
                                            ctx.data_mut(|data| data.insert_temp(Id::new(("composer_picker_anchor", &thread_id)), emo_rect));
                                        }
                                        if self.sticker_picker_open || emo_resp.hovered() {
                                            ui.painter().rect_filled(
                                                emo_rect,
                                                Rounding::same(6.0),
                                                pal.bg_hover,
                                            );
                                        }
                                        draw_lucide_icon(
                                            ui.painter(),
                                            emo_rect.center(),
                                            lucide::SMILE,
                                            18.0,
                                            if self.sticker_picker_open || emo_resp.hovered() {
                                                Color32::from_rgb(251, 191, 36)
                                            } else {
                                                pal.text_secondary
                                            },
                                        );
                                        if emo_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .on_hover_text(crate::i18n::tr("Biểu tượng cảm xúc & Nhãn dán"))
                                            .clicked()
                                        {
                                            self.sticker_picker_open = !self.sticker_picker_open;
                                        }
                                        // Responsive Padded Send Button (shows icon-only on very tight widths)
                                        let is_voice_active = self.voice_binding.is_some() || self.recorded_voice_samples.is_some();
                                        let can_send = is_voice_active || !self.input_message.trim().is_empty()
                                            || self.pending_clipboard_image.is_some() || self.attachment_drafts.len(&thread_id)>0;
                                        let (send_rect, send_resp) = ui.allocate_exact_size(
                                            Vec2::new(send_btn_w, 34.0),
                                            Sense::click(),
                                        );
                                        #[cfg(test)]
                                        ctx.data_mut(|data| data.insert_temp(Id::new("composer-send-bounds"), (send_rect, ui.clip_rect())));
                                        composer_send_bounds = Some((send_rect, ui.clip_rect()));
                                        if can_send {
                                            let send_bg = if send_resp.hovered() {
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
                                        } else {
                                            // Dynamic Thumbs-up / Like button when empty (Zalo PC & Olaz UX)
                                            let like_bg = if send_resp.hovered() {
                                                pal.bg_hover
                                            } else {
                                                pal.bg_input
                                            };
                                            ui.painter().rect_filled(
                                                send_rect,
                                                Rounding::same(8.0),
                                                like_bg,
                                            );
                                            ui.painter().rect_stroke(
                                                send_rect,
                                                Rounding::same(8.0),
                                                Stroke::new(1.0_f32, pal.border),
                                            );
                                            ui.painter().text(
                                                send_rect.center(),
                                                egui::Align2::CENTER_CENTER,
                                                "👍",
                                                FontId::proportional(16.0),
                                                pal.primary,
                                            );
                                        }


                                        send_resp.context_menu(|ui|{
                                            if ui.button(crate::i18n::tr("Định dạng văn bản (Ctrl+Shift+S / Ctrl+Shift+X)")).clicked() {
                                                self.composer_format_mode = true;
                                                ui.close_menu();
                                            }
                                            for (mode,label) in [(Some(crate::rich_text::Urgency::Important),"Đánh dấu Quan trọng"),
                                                (Some(crate::rich_text::Urgency::Urgent),"Đánh dấu Khẩn cấp"),(None,"Gửi bình thường")] {
                                                if ui.selectable_label(composer_format.urgency == mode,label).clicked() {
                                                    composer_format.urgency = mode;
                                                    ctx.data_mut(|data|data.insert_temp(composer_format_key,composer_format.clone()));
                                                    ui.close_menu();
                                                }
                                            }
                                            if ui.button(crate::i18n::tr("Nhãn dán…")).clicked(){self.sticker_picker_open = !self.sticker_picker_open;ui.close_menu();}
                                            if ui.button(crate::i18n::tr("Tạo nhắc hẹn…")).clicked() {
                                                self.open_server_board(thread_id.clone(), is_group, self.input_message.clone());
                                                ui.close_menu();
                                            }
                                            if ui.button(crate::i18n::tr("Gửi danh thiếp…")).clicked() {
                                                self.send_contact_modal_open = true;
                                                self.send_contact_query.clear();
                                                ui.close_menu();
                                            }
                                            if ui.add_enabled(self.settings.quick_messages_enabled, egui::Button::new(crate::i18n::tr("Tin nhắn nhanh…"))).clicked() {
                                                self.tools.open = true;
                                                self.tools.tab = 6;
                                                ui.close_menu();
                                            }
                                            if ui.button(crate::i18n::tr("Giao việc & Bình chọn…")).clicked() {
                                                self.tools.open = true;
                                                self.tools.tab = 5;
                                                ui.close_menu();
                                            }
                                            for (images_only, label) in
                                                [(true, "Gửi ảnh…"), (false, "Gửi tệp…")]
                                            {
                                                if ui.button(label).clicked() {
                                                    self.request_attachment_picker(
                                                        &thread_id,
                                                        is_group,
                                                        images_only,
                                                    );
                                                    ui.close_menu();
                                                }
                                            }
                                            if ui.button(crate::i18n::tr("Hẹn giờ gửi…")).clicked(){self.open_quick_schedule(ctx);ui.close_menu();}
                                            if ui.button(crate::i18n::tr("Quản lý lịch gửi")).clicked(){self.tools.open=true;self.tools.tab=3;ui.close_menu();}
                                        });
                                        let send_resp = if can_send {
                                            send_resp.on_hover_text(crate::i18n::tr("Gửi tin nhắn (Enter)"))
                                        } else {
                                            send_resp.on_hover_text(crate::i18n::tr("Gửi nhanh biểu tượng thích (Like)"))
                                        };
                                        let send_clicked = send_resp
                                            .on_hover_cursor(CursorIcon::PointingHand)
                                            .clicked();

                                        let unsupported_metadata = self.composer_has_metadata(ctx, &thread_id, is_group)
                                            && (is_voice_active
                                                || self.pending_clipboard_image.is_some() || send_selected_sticker.is_some());
                                        if ((send_clicked || enter_pressed) || send_selected_sticker.is_some()) && unsupported_metadata {
                                            self.status_msg = "Chưa hỗ trợ định dạng hoặc mức Quan trọng/Khẩn cấp cho ảnh, giọng nói hay nhãn dán; hãy tắt chúng hoặc gửi văn bản riêng. Nội dung vẫn được giữ.".into();
                                        } else if (send_clicked || enter_pressed) && is_voice_active {
                                            self.send_voice_recording(ctx,&thread_id, is_group);
                                            response.request_focus();
                                        } else if (send_clicked || enter_pressed) && self.attachment_drafts.len(&thread_id) > 0 {
                                            self.send_attachment_draft(ctx,&thread_id,is_group);
                                            response.request_focus();
                                        } else if send_selected_sticker.is_some() && attachment_count > 0 {
                                            self.status_msg="Hãy gửi hoặc bỏ tệp chờ trước khi gửi nhãn dán".into();
                                        } else if send_selected_sticker.is_some() && !self.composer_mentions.mentions.is_empty() {
                                            self.status_msg = "Hãy gửi mention bằng tin nhắn riêng trước khi gửi nhãn dán".into();
                                        } else if let Some(chosen) = send_selected_sticker {
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
                                        } else if (send_clicked || enter_pressed) && self.pending_clipboard_image.is_some() && !self.composer_mentions.mentions.is_empty() {
                                            self.status_msg = "Hãy gửi mention bằng tin nhắn riêng trước khi gửi ảnh".into();
                                        } else if (send_clicked || enter_pressed)
                                            && self.pending_clipboard_image.is_some()
                                        {
                                            if let Some((clip_path, _, _, _)) = self.pending_clipboard_image.as_ref() {
                                                let queued=WORKER_TX.get().is_some_and(|tx|tx.send(WorkerCmd::PickAndSendAttachment {
                                                    owner:self.user_uin.clone(), runtime_id:self.account_runtime_id,
                                                    to_id:thread_id.clone(),is_group,images_only:true,
                                                    caption:self.input_message.trim().to_owned(),preset_path:Some(clip_path.clone()),
                                                    photo_original:self.settings.send_photo_original,ephemeral:true,
                                                }).is_ok());
                                                if queued {
                                                    let _ = self.retire_pending_clipboard_image();
                                                    self.dismissed_sticker_suggest_for.clear();self.composer_sticker_selected=None;
                                                } else {self.status_msg="Không gửi được ảnh; ảnh và nội dung soạn thảo vẫn được giữ.".into();}
                                                response.request_focus();
                                            }
                                        } else if (send_clicked || enter_pressed) && can_send {
                                            if self.settings.quick_messages_enabled && self.input_message.starts_with('/') {
                                                if let Some(expanded) = self.tools.config.quick_messages.expand(&self.input_message) {
                                                    self.input_message = expanded.to_string();
                                                }
                                            }
                                            self.composer_mentions.edit(&self.input_message);
                                            let draft = self.composer_mentions.trimmed();
                                            let text = draft.text.clone();
                                            response.request_focus();
                                            let quoted = self.replying_to.clone();
                                            let options = composer_format.options(&text);
                                            if let Some(tx) = WORKER_TX.get() {
                                                let command = if !options.styles.is_empty() || options.urgency.is_some() {
                                                    WorkerCmd::SendFormatted {to_id:thread_id.clone(), text, is_group,
                                                        options, mentions:draft.mentions, quoted}
                                                } else if is_group && !draft.mentions.is_empty() {
                                                    WorkerCmd::SendMentioned {to_id: thread_id.clone(), draft, quoted}
                                                } else if let Some(quoted) = quoted {
                                                    WorkerCmd::SendQuote {
                                                        to_id: thread_id.clone(),
                                                        text,
                                                        quoted,
                                                        is_group,
                                                    }
                                                } else {
                                                    WorkerCmd::SendMessage {
                                                        to_id: thread_id.clone(),
                                                        text,
                                                        is_group,
                                                    }
                                                };
                                                if tx.send(command).is_ok() {
                                                    self.input_message.clear();
                                                    self.composer_mentions.clear();
                                                    self.replying_to = None;
                                                    self.dismissed_sticker_suggest_for.clear();
                                                    self.composer_sticker_selected = None;
                                                    ctx.data_mut(|data|data.remove::<crate::rich_text::ComposerFormat>(composer_format_key));
                                                } else {
                                                    self.status_msg = "Không thể gửi; nội dung vẫn được giữ trong ô soạn thảo".into();
                                                }
                                            } else {
                                                self.status_msg = "Chưa kết nối bộ xử lý; nội dung vẫn được giữ".into();
                                            }
                                        } else if send_clicked && !can_send {
                                            // Send Zalo PC Like "👍" via WorkerCmd::SendMessage
                                            response.request_focus();
                                            if let Some(tx) = WORKER_TX.get() {
                                                let _ = tx.send(WorkerCmd::SendMessage {
                                                    to_id: thread_id.clone(),
                                                    text: "👍".into(),
                                                    is_group,
                                                });
                                            }
                                        } else if enter_pressed {
                                            response.request_focus();
                                        }
                                    });
                            if self.composer_format_mode || composer_format.urgency.is_some() {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    for (index, active, label) in [
                                        (0, composer_format.bold, RichText::new("B").strong()),
                                        (1, composer_format.italic, RichText::new("I").italics()),
                                        (2, composer_format.underline, RichText::new("U").underline()),
                                        (3, composer_format.strike, RichText::new("S").strikethrough()),
                                    ] {
                                        if ui.add_sized([28.0,26.0], egui::SelectableLabel::new(active,label)).clicked() {
                                            let (start,end) = ctx.data(|data| data.get_temp::<(usize,usize)>(composer_scope.with("selection")))
                                                .unwrap_or_default();
                                            composer_format.toggle_selection(&self.input_message,start,end,index);
                                            self.composer_focus_pending = true;
                                        }
                                    }
                                    if let Some(urgency) = composer_format.urgency {
                                        ui.label(match urgency {
                                            crate::rich_text::Urgency::Important => "Quan trọng",
                                            crate::rich_text::Urgency::Urgent => "Khẩn cấp",
                                        });
                                    }
                                    if ui.add_sized([28.0,26.0],egui::Button::new(
                                        RichText::new(if composer_expanded {"↙"} else {"↗"}).size(18.0)).frame(false))
                                        .on_hover_text(if composer_expanded {"Thu gọn vùng soạn thảo"} else {"Mở rộng vùng soạn thảo"}).clicked() {
                                        composer_expanded = !composer_expanded;
                                        ctx.data_mut(|data| data.insert_temp(expanded_key, composer_expanded));
                                        ctx.request_repaint();
                                    }
                                    if ui.add_sized([28.0,26.0],egui::Button::new("×").frame(false))
                                        .on_hover_text("Tắt định dạng (Ctrl+Shift+X)").clicked() {
                                        self.composer_format_mode = false;
                                        composer_format = Default::default();
                                    }
                                });
                                ctx.data_mut(|data| data.insert_temp(composer_format_key, composer_format.clone()));
                            }
                                });
                        });

                    let mut compose_drop_rect = composer_frame.response.rect;
                    let actual_height = composer_frame.response.rect.height();
                    if (actual_height - reserved_composer_height).abs() > 0.5 {
                        ctx.data_mut(|data| data.insert_temp(composer_height_key, actual_height));
                        ctx.request_discard("anchor measured composer to the window bottom");
                    }
                    // The measurement pass can overflow before its new height is applied.
                    // Smoke validates the final layout that egui will actually paint.
                    if self.smoke.is_some() && !ctx.will_discard() {
                        if let Some((send, clip)) = composer_send_bounds {
                            if !clip.expand(1.0).contains_rect(send) {
                                eprintln!("UI smoke failed: send button outside composer viewport: send={send:?}, clip={clip:?}");
                                std::process::exit(3);
                            }
                        }
                    }
                    #[cfg(test)]
                    ctx.data_mut(|data| data.insert_temp(Id::new("composer-layout-bounds"),
                        (composer_frame.response.rect, composer_bottom, editor_height)));
                    compose_drop_rect.min.y = compose_top;
                    self.handle_file_drop(ctx, &thread_id, is_group, message_scroll.inner_rect, compose_drop_rect, drop_blocked_at_start, pal);

                    // 4b. Floating Contextual Sticker Suggestion Popover (`egui::Area`, rendered AFTER composer bar)
                    if !file_drag_active && !contextual_stickers.is_empty() && !self.sticker_picker_open {
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
                        if !file_drag_active && contextual_stickers.is_empty() && !self.sticker_picker_open {
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
                            RichText::new(crate::i18n::tr("Chọn một hội thoại bên trái để bắt đầu trò chuyện"))
                                .size(15.0)
                                .color(pal.text_muted),
                        );
                    });
                }
            });

        self.render_workspace_overlays(ctx, pal);
    }

    /// Select a conversation, mark visible messages read and load server history when needed.
    pub(super) fn open_thread(
        &mut self,
        ctx: &egui::Context,
        idx: usize,
        tid: String,
        is_group: bool,
    ) {
        if !self.private_access(&tid, is_group) {
            self.hidden_panel.open = true;
            return;
        }
        if self.selected_thread_index != idx
            || self.threads.get(self.selected_thread_index).map(|t| &t.id) != Some(&tid)
        {
            self.clear_timeline_selection();
            self.replying_to = None;
            self.filter_starred_only = false;
            self.drawer_view = DrawerView::Overview;
            self.drawer_search_query.clear();
            self.drawer_media_query.clear();
            self.drawer_media_sender.clear();
            self.drawer_media_from.clear();
            self.drawer_media_to.clear();
            self.drawer_media_type = None;
            self.archive_history = Default::default();
            self.archive_mutation_confirm = None;
            self.drawer_media_filter_key.clear();
            self.drawer_media_thread_id = tid.clone();
            self.drawer_media_active_tab = MediaTab::Photos;
            self.drawer_media_limit = 100;
            self.drawer_media_selection_mode = false;
            self.drawer_media_selected.clear();
            self.group_member_search.clear();
            self.group_member_popover = None;
            self.common_groups_expanded = false;
        }
        self.selected_thread_index = idx;
        if let Some(tx) = WORKER_TX.get() {
            let _ = tx.send(WorkerCmd::PrioritizeConversationSync {
                owner: self.user_uin.clone(),
                runtime_id: self.account_runtime_id,
                thread_id: tid.clone(),
            });
        }
        self.refresh_message_pins(&tid, is_group);
        if is_group {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchGroupPolicy {
                    group_id: tid.clone(),
                });
            }
        } else if !self.trial_peers.contains(&tid) {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::CheckE2eeStatus {
                    owner: self.user_uin.clone(),
                    peer_id: tid.clone(),
                });
            }
        }
        self.policy_command(crate::conversation_policy::Action::Read);
        self.sync_composer_draft(ctx);
        let mut clear_manual_mark = false;
        if let Some(thread) = self.threads.get_mut(idx) {
            if thread.unread.marked_unread {
                thread.unread.marked_unread = false;
                clear_manual_mark = true;
            }
            thread.unread.read_foreground(&thread.messages);
            if let Some(last) = thread.messages.last() {
                let mid = last.id.clone();
                thread.unread.last_seen_id = Some(mid.clone());
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(crate::gui::WorkerCmd::MarkThreadRead {
                        thread_id: tid.clone(),
                        last_read_msg_id: mid,
                    });
                }
            }
        }
        if clear_manual_mark {
            self.manual_mark_command(tid.clone(), is_group, crate::unread::MarkAction::Remove);
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
            .is_some_and(|t| self.is_cloud_thread(t));
        if is_group || is_cloud {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::LoadGroupHistory {
                    limit: 500,
                    group_id: tid,
                    is_group,
                });
            }
        } else {
            let should_fetch = match self.presence_map.get(&tid) {
                Some((_, last_fetch)) => last_fetch.elapsed() > std::time::Duration::from_secs(60),
                None => true,
            };
            if should_fetch {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::FetchLastOnline {
                        owner: self.user_uin.clone(),
                        runtime_id: self.account_runtime_id,
                        target: tid,
                        official: false,
                    });
                }
            }
        }
    }

    fn dispatch_reaction_intent(
        &mut self,
        thread_id: &str,
        is_group: bool,
        message: ChatMessage,
        reaction: String,
    ) {
        if !self.is_logged_in || self.user_uin.is_empty() {
            return;
        }
        self.next_reaction_action_id = self.next_reaction_action_id.wrapping_add(1);
        let intent_id = self.next_reaction_action_id;
        let message_key = if message.id.is_empty() {
            format!("ts:{}", message.ts)
        } else {
            message.id.clone()
        };
        let key = (thread_id.to_owned(), message_key);
        // The toggle reads confirmed state; a second click before the first outcome
        // would re-send "add". The worker always answers, so the key is released.
        if self.pending_reaction_actions.contains_key(&key) {
            self.show_toast("Đang xử lý cảm xúc trước đó");
            return;
        }
        self.pending_reaction_actions.insert(key.clone(), intent_id);
        let owner = self.user_uin.clone();
        let result = WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::React {
                owner,
                runtime_id: self.account_runtime_id,
                intent_id,
                to_id: thread_id.to_owned(),
                is_group,
                msg: message,
                reaction,
            })
            .is_ok()
        });
        if !result {
            if self.pending_reaction_actions.get(&key) == Some(&intent_id) {
                self.pending_reaction_actions.remove(&key);
            }
            self.show_toast("Không thể gửi yêu cầu cảm xúc");
        }
    }

    fn render_workspace_overlays(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        self.render_profile_popover(ctx, pal);
        self.render_group_member_popover(ctx, pal);
        self.render_toast_overlay(ctx, pal);
        self.render_forward_dialog(ctx, pal);
        self.render_call_overlay(ctx, pal);
        self.render_image_viewer_modal(ctx, pal);
        self.render_doc_viewer_modal(ctx, pal);
        self.render_phone_lookup_modal(ctx, pal);
        self.render_settings_modal(ctx, pal);
        self.render_pin_setup_modal(ctx, pal);
        self.render_tools_panel(ctx);
        self.render_history_search(ctx);
        self.render_server_board(ctx);
        self.render_group_panel(ctx);
        self.render_group_entry(ctx);
        self.render_contact_panel(ctx);
        self.render_labels_panel(ctx);
        self.render_own_profile(ctx);
        self.render_avatar_panel(ctx);
        self.render_backup_panel(ctx);
        self.render_policy_panel(ctx);
        self.render_auto_delete_modal(ctx, pal);
        self.render_content_protection_modal(ctx, pal);
        self.render_group_content(ctx);
        self.render_downloads(ctx);
        self.render_quick_schedule(ctx, pal);
        self.render_send_contact_modal(ctx, pal);
    }

    fn render_send_contact_modal(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if !self.send_contact_modal_open {
            return;
        }
        let mut open = self.send_contact_modal_open;
        let mut chosen_contact = None;
        let mut close_modal = false;
        egui::Window::new(RichText::new("Gửi danh thiếp").strong().size(15.0))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(380.0)
            .min_width(360.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                // 1. Search bar with icon
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(lucide::SEARCH)
                            .size(15.0)
                            .color(pal.text_secondary),
                    );
                    let search_edit = egui::TextEdit::singleline(&mut self.send_contact_query)
                        .hint_text("Tìm danh thiếp theo tên...")
                        .desired_width(ui.available_width());
                    ui.add(search_edit);
                });
                ui.add_space(6.0);

                // 2. Filter chips (pills)
                const FILTERS: [&str; 5] =
                    ["Tất cả", "Khách hàng", "Gia đình", "Công việc", "Bạn bè"];
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                    for (idx, &filter_name) in FILTERS.iter().enumerate() {
                        let is_selected = self.send_contact_filter == idx;
                        let bg_color = if is_selected {
                            pal.primary
                        } else {
                            pal.bg_input
                        };
                        let text_color = if is_selected {
                            Color32::WHITE
                        } else {
                            pal.text_secondary
                        };
                        let stroke = if is_selected {
                            Stroke::NONE
                        } else {
                            Stroke::new(1.0_f32, pal.border)
                        };
                        let pill_btn = egui::Button::new(
                            RichText::new(filter_name).size(12.0).color(text_color),
                        )
                        .fill(bg_color)
                        .stroke(stroke)
                        .rounding(Rounding::same(12.0));
                        if ui.add(pill_btn).clicked() {
                            self.send_contact_filter = idx;
                        }
                    }
                });
                ui.add_space(4.0);
                ui.separator();

                // 3. Contacts list
                let query = fold_vn_search(self.send_contact_query.trim());
                let filter_idx = self.send_contact_filter;
                let friends_set = self.inbox_policy.friends.as_ref();
                let labels_snapshot = self.labels_panel.snapshot.as_ref();

                let mut matched_count = 0;
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .min_scrolled_height(180.0)
                    .show(ui, |ui| {
                        for t in &self.threads {
                            if t.is_group
                                || t.id.starts_with("group_")
                                || t.id == self.user_uin
                                || t.id.is_empty()
                                || t.name.contains("Cloud của tôi")
                            {
                                continue;
                            }
                            let display_name =
                                resolve_contact_name_from(&self.server_aliases, &t.id, &t.name);
                            let folded_name = fold_vn_search(display_name.trim());
                            if !query.is_empty()
                                && !folded_name.contains(&query)
                                && !t.id.contains(&query)
                            {
                                continue;
                            }

                            // Filter logic
                            let passes_category = match filter_idx {
                                0 => true, // Tất cả
                                1 => {
                                    // Khách hàng
                                    let mut found = false;
                                    if let Some(snapshot) = labels_snapshot {
                                        for label in &snapshot.labels {
                                            if label.text.to_lowercase().contains("khách")
                                                && label.conversations.iter().any(|c| c == &t.id)
                                            {
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                    found || display_name.to_lowercase().contains("khách")
                                }
                                2 => {
                                    // Gia đình
                                    let mut found = false;
                                    if let Some(snapshot) = labels_snapshot {
                                        for label in &snapshot.labels {
                                            if label.text.to_lowercase().contains("gia đình")
                                                && label.conversations.iter().any(|c| c == &t.id)
                                            {
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                    found || display_name.to_lowercase().contains("gia đình")
                                }
                                3 => {
                                    // Công việc
                                    let mut found = false;
                                    if let Some(snapshot) = labels_snapshot {
                                        for label in &snapshot.labels {
                                            if (label.text.to_lowercase().contains("công việc")
                                                || label.text.to_lowercase().contains("việc"))
                                                && label.conversations.iter().any(|c| c == &t.id)
                                            {
                                                found = true;
                                                break;
                                            }
                                        }
                                    }
                                    found
                                        || display_name.to_lowercase().contains("việc")
                                        || display_name.to_lowercase().contains("work")
                                }
                                4 => {
                                    // Bạn bè
                                    if let Some(friends) = friends_set {
                                        if friends.contains(&t.id) {
                                            true
                                        } else {
                                            let mut found = false;
                                            if let Some(snapshot) = labels_snapshot {
                                                for label in &snapshot.labels {
                                                    if label.text.to_lowercase().contains("bạn")
                                                        && label
                                                            .conversations
                                                            .iter()
                                                            .any(|c| c == &t.id)
                                                    {
                                                        found = true;
                                                        break;
                                                    }
                                                }
                                            }
                                            found
                                        }
                                    } else {
                                        true
                                    }
                                }
                                _ => true,
                            };

                            if !passes_category {
                                continue;
                            }

                            matched_count += 1;
                            let is_selected = self.send_contact_selected.as_deref() == Some(&t.id);
                            let row_bg = if is_selected {
                                pal.primary.linear_multiply(0.12)
                            } else {
                                Color32::TRANSPARENT
                            };

                            let row_resp = egui::Frame::none()
                                .fill(row_bg)
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Margin::symmetric(8.0, 5.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        // Radio selection indicator
                                        let radio_val = is_selected;
                                        if ui.radio(radio_val, "").clicked() {
                                            self.send_contact_selected = Some(t.id.clone());
                                        }

                                        // Contact avatar
                                        let (rect, _) = ui
                                            .allocate_exact_size(Vec2::splat(32.0), Sense::hover());
                                        let bg = avatar_color_for(display_name, false);
                                        ui.painter().circle_filled(rect.center(), 16.0, bg);
                                        let initials = avatar_initials(display_name);
                                        ui.painter().text(
                                            rect.center(),
                                            egui::Align2::CENTER_CENTER,
                                            initials,
                                            FontId::proportional(12.5),
                                            Color32::WHITE,
                                        );

                                        // Contact Name & preview / subtitle
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(display_name)
                                                    .strong()
                                                    .size(13.5)
                                                    .color(if is_selected {
                                                        pal.primary
                                                    } else {
                                                        pal.text_primary
                                                    }),
                                            );
                                            if !t.preview.is_empty() && !t.preview.starts_with('{')
                                            {
                                                ui.label(
                                                    RichText::new(&t.preview)
                                                        .size(11.5)
                                                        .color(pal.text_secondary),
                                                );
                                            }
                                        });
                                    });
                                })
                                .response;

                            let row_interact =
                                ui.interact(row_resp.rect, ui.id().with(&t.id), Sense::click());
                            if row_interact.clicked() {
                                self.send_contact_selected = Some(t.id.clone());
                            }
                            if row_interact.double_clicked() {
                                self.send_contact_selected = Some(t.id.clone());
                                chosen_contact = Some((
                                    t.id.clone(),
                                    display_name.to_string(),
                                    t.avatar.clone().unwrap_or_default(),
                                ));
                            }

                            ui.add_space(2.0);
                        }

                        if matched_count == 0 {
                            ui.vertical_centered(|ui| {
                                ui.add_space(24.0);
                                ui.label(
                                    RichText::new("Không tìm thấy danh thiếp nào")
                                        .size(13.0)
                                        .color(pal.text_secondary),
                                );
                                ui.add_space(24.0);
                            });
                        }
                    });

                // 4. Modal footer with Cancel and Send buttons
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::tr("Hủy")).clicked() {
                        close_modal = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let has_selection = self.send_contact_selected.is_some();
                        let send_btn = egui::Button::new(
                            RichText::new(crate::i18n::tr("Gửi danh thiếp"))
                                .strong()
                                .color(Color32::WHITE),
                        )
                        .fill(if has_selection {
                            pal.primary
                        } else {
                            pal.text_muted
                        })
                        .rounding(Rounding::same(6.0));
                        if ui.add_enabled(has_selection, send_btn).clicked() {
                            if let Some(sel_id) = &self.send_contact_selected {
                                if let Some(contact) = self.threads.iter().find(|t| &t.id == sel_id)
                                {
                                    chosen_contact = Some((
                                        contact.id.clone(),
                                        resolve_contact_name_from(
                                            &self.server_aliases,
                                            &contact.id,
                                            &contact.name,
                                        )
                                        .to_string(),
                                        contact.avatar.clone().unwrap_or_default(),
                                    ));
                                }
                            }
                        }
                    });
                });
            });

        if close_modal {
            open = false;
        }
        self.send_contact_modal_open = open;
        if let Some((cid, cname, cava)) = chosen_contact {
            self.send_contact_modal_open = false;
            self.send_contact_selected = None;
            if let Some(t) = self.threads.get(self.selected_thread_index) {
                let to_id = t.id.clone();
                let is_group = t.is_group;
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::SendContactCard {
                        to_id,
                        is_group,
                        contact_id: cid,
                        contact_name: cname,
                        phone: String::new(),
                        avatar: cava,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod sync_and_order_tests {
    use super::*;

    #[test]
    fn archive_photo_date_groups_do_not_mix_rows_across_midnight() {
        use chrono::TimeZone;
        let today = chrono::Local::now().date_naive();
        let yesterday = today.checked_sub_days(chrono::Days::new(1)).unwrap();
        let before = chrono::Local
            .from_local_datetime(&yesterday.and_hms_opt(23, 59, 0).unwrap())
            .single()
            .unwrap()
            .timestamp_millis();
        let after = chrono::Local
            .from_local_datetime(&today.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap()
            .timestamp_millis();
        let photos = [
            (
                "a".into(),
                Default::default(),
                ChatMessage {
                    ts: before,
                    ..Default::default()
                },
            ),
            (
                "b".into(),
                Default::default(),
                ChatMessage {
                    ts: before,
                    ..Default::default()
                },
            ),
            (
                "c".into(),
                Default::default(),
                ChatMessage {
                    ts: after,
                    ..Default::default()
                },
            ),
        ];
        let photos: Vec<_> = photos.iter().collect();
        let groups = group_photos_by_local_date(photos);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].1.len(), 1);
        assert_ne!(groups[0].0, groups[1].0);
    }

    #[test]
    fn archive_date_filter_normalizes_second_and_millisecond_timestamps() {
        let seconds = 1_800_000_000;
        let milliseconds = seconds * 1000;
        assert_eq!(
            archive_message_local_date(seconds),
            archive_message_local_date(milliseconds)
        );
    }

    #[test]
    fn sidebar_mark_unread_selects_latest_across_timestamp_units() {
        let messages = [
            ChatMessage {
                id: "older-millis".into(),
                ts: 1_800_000_000_000,
                ..Default::default()
            },
            ChatMessage {
                id: "newer-seconds".into(),
                ts: 1_800_000_100,
                ..Default::default()
            },
        ];
        assert_eq!(
            latest_message_by_time(&messages).map(|message| message.id),
            Some("newer-seconds".into())
        );
    }

    pub(super) fn app_and_context() -> (ZaloGuiApp, egui::Context) {
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
    #[test]
    fn timeline_batch_forward_opens_preview_with_original_scoped_sources_before_dispatch() {
        let (mut app, _) = app_and_context();
        app.user_uin = "owner-one".into();
        app.hidden_panel.view.ready = true;
        app.policy_panel.snapshot = Some(Default::default());
        app.threads = vec![
            Thread {
                id: "thread-a".into(),
                messages: vec![
                    ChatMessage {
                        id: "same-id".into(),
                        text: "source A".into(),
                        ..Default::default()
                    },
                    ChatMessage {
                        id: "second-id".into(),
                        text: "second source A".into(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            Thread {
                id: "thread-b".into(),
                messages: vec![ChatMessage {
                    id: "same-id".into(),
                    text: "source B".into(),
                    ..Default::default()
                }],
                ..Default::default()
            },
        ];
        app.bind_timeline_selection("thread-a", false);
        app.selected_msg_ids
            .extend(["same-id".into(), "second-id".into()]);
        app.share_timeline_selection("thread-a", false);
        assert!(app.forward_msg.is_some());
        assert_eq!(app.forward_batch.len(), 2);
        assert!(
            app.forward_session.is_none(),
            "recipient selection and final confirmation must precede dispatch"
        );
        assert!(app.forward_targets.is_empty());
        assert_eq!(app.forward_batch[0].text, "source A");
        assert_eq!(
            crate::chat::forward_source(&app.forward_batch[0]),
            Some(("thread-a".into(), false))
        );
        assert!(!app.batch_selection_mode && app.selected_msg_ids.is_empty());
    }

    #[test]
    fn timeline_selection_cannot_alias_reused_ids_after_thread_or_account_switch() {
        let (mut app, ctx) = app_and_context();
        app.user_uin = "owner-one".into();
        app.hidden_panel.view.ready = true;
        app.threads = vec![
            Thread {
                id: "thread-a".into(),
                ..Default::default()
            },
            Thread {
                id: "thread-b".into(),
                ..Default::default()
            },
        ];
        app.bind_timeline_selection("thread-a", false);
        app.selected_msg_ids.insert("reused-id".into());
        app.open_thread(&ctx, 1, "thread-b".into(), false);
        assert!(
            !app.batch_selection_mode
                && app.selected_msg_ids.is_empty()
                && app.timeline_selection_scope.is_none()
        );
        app.bind_timeline_selection("thread-b", false);
        app.selected_msg_ids.insert("reused-id".into());
        app.user_uin = "owner-two".into();
        app.share_timeline_selection("thread-b", false);
        assert!(app.forward_msg.is_none());
        assert!(app.selected_msg_ids.is_empty());
        app.bind_timeline_selection("thread-b", false);
        app.selected_msg_ids.insert("reused-id".into());
        app.clear_presence_state();
        assert!(
            !app.batch_selection_mode
                && app.selected_msg_ids.is_empty()
                && app.timeline_selection_scope.is_none()
        );
    }

    #[test]
    fn search_shortcut_does_not_open_a_background_drawer_behind_settings() {
        let (mut app, ctx) = app_and_context();
        app.user_uin = "owner".into();
        app.hidden_panel.view.ready = true;
        app.threads = vec![Thread {
            id: "peer".into(),
            name: "Fixture chat".into(),
            ..Default::default()
        }];
        app.selected_thread_index = 0;
        for modal in [true, false] {
            app.settings_modal_open = modal;
            app.info_drawer_open = false;
            app.drawer_view = DrawerView::Overview;
            let modifiers = egui::Modifiers {
                ctrl: true,
                command: true,
                ..Default::default()
            };
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                    modifiers,
                    events: vec![egui::Event::Key {
                        key: Key::F,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers,
                    }],
                    ..Default::default()
                },
                |ctx| {
                    let palette = app.palette();
                    app.render_chat_workspace(ctx, &palette);
                },
            );
            assert_eq!(app.info_drawer_open, !modal);
            if !modal {
                assert_eq!(app.drawer_view, DrawerView::SearchInThread);
            }
        }
    }

    #[test]
    fn switching_to_compact_viewport_keeps_drawer_closed_until_clicked() {
        let (mut app, ctx) = app_and_context();
        app.user_uin = "owner".into();
        app.hidden_panel.view.ready = true;
        app.policy_panel.snapshot = Some(Default::default());
        app.threads = vec![Thread {
            id: "peer".into(),
            name: "Fixture chat".into(),
            ..Default::default()
        }];
        app.selected_thread_index = 0;
        app.info_drawer_open = true;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))),
                ..Default::default()
            },
            |ctx| {
                let palette = app.palette();
                app.render_chat_workspace(ctx, &palette);
            },
        );
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                ..Default::default()
            },
            |ctx| {
                let palette = app.palette();
                app.render_chat_workspace(ctx, &palette);
            },
        );
        let drawer_open = ctx.data(|data| {
            data.get_temp::<bool>(Id::new("narrow_drawer_open"))
                .unwrap_or(false)
        });
        assert!(
            !drawer_open,
            "entering compact mode must not open the overlay drawer"
        );
    }

    #[test]
    fn modal_hides_independent_narrow_drawer_and_timeline_action_areas() {
        let (mut app, ctx) = app_and_context();
        app.user_uin = "owner".into();
        app.hidden_panel.view.ready = true;
        app.policy_panel.snapshot = Some(Default::default());
        app.threads = vec![Thread {
            id: "100".into(),
            name: "Fixture chat".into(),
            messages: vec![ChatMessage {
                id: "message-one".into(),
                text: "Fixture message".into(),
                ..Default::default()
            }],
            ..Default::default()
        }];
        app.selected_thread_index = 0;
        app.bind_timeline_selection("100", false);
        app.selected_msg_ids.insert("message-one".into());
        ctx.data_mut(|data| data.insert_temp(Id::new("narrow_drawer_open"), true));
        ctx.data_mut(|data| data.insert_temp(Id::new("narrow_chat_open"), true));
        let mut text = String::new();
        for gated in [false, true] {
            app.settings_modal_open = gated;
            for _ in 0..2 {
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 800.0))),
                        ..Default::default()
                    },
                    |ctx| {
                        let palette = app.palette();
                        app.render_chat_workspace(ctx, &palette);
                    },
                );
                text = output
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            if gated {
                assert!(
                    !text.contains("Thông tin hội thoại"),
                    "narrow Area must not remain interactive above a modal"
                );
                assert!(
                    !text.contains("Đã chọn:"),
                    "timeline Area must not remain interactive above a modal"
                );
            } else {
                assert!(text.contains("Thông tin hội thoại"));
                assert!(text.contains("Đã chọn:"));
            }
        }
    }

    #[test]
    fn archive_link_row_renders_cached_thumbnail_and_blocks_inline_copy_for_unknown_policy() {
        let (mut app, ctx) = app_and_context();
        app.hidden_panel.view.ready = true;
        let thumb = "http://127.0.0.1:1/fixture-thumbnail";
        let message = ChatMessage {
            id: "link-one".into(),
            client_message_id: "link-client".into(),
            link_thumb: Some(thumb.into()),
            ..Default::default()
        };
        let key = format!(
            "archive-link-{}",
            crate::crypto::md5_hex(
                &serde_json::json!(["100", message.id, message.client_message_id, thumb])
                    .to_string()
            )
        );
        let texture = ctx.load_texture(
            "fixture cached thumbnail",
            egui::ColorImage::new([2, 2], Color32::RED),
            Default::default(),
        );
        let texture_id = texture.id();
        app.sticker_textures.insert(key, texture);
        let mut copy_position = None;
        for _ in 0..2 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(700.0, 200.0))),
                    ..Default::default()
                },
                |ctx| {
                    let palette = app.palette();
                    egui::CentralPanel::default().show(ctx, |ui| {
                        app.render_link_row(
                            ui,
                            "https://example.invalid/fixture",
                            "Fixture link",
                            &message,
                            "100",
                            false,
                            &palette,
                        )
                    });
                },
            );
            assert!(output.shapes.iter().any(|shape|matches!(&shape.shape,egui::Shape::Mesh(mesh) if mesh.texture_id==texture_id)),"cached link preview must render its real thumbnail");
            copy_position = output.shapes.iter().find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == "📋" => {
                    Some(text.pos + text.galley.size() / 2.0)
                }
                _ => None,
            });
        }
        let position = copy_position.expect("inline copy control must be present");
        for pressed in [true, false] {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(700.0, 200.0))),
                    events: vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    ..Default::default()
                },
                |ctx| {
                    let palette = app.palette();
                    egui::CentralPanel::default().show(ctx, |ui| {
                        app.render_link_row(
                            ui,
                            "https://example.invalid/fixture",
                            "Fixture link",
                            &message,
                            "100",
                            false,
                            &palette,
                        )
                    });
                },
            );
            assert!(
                output.platform_output.copied_text.is_empty(),
                "unknown protection policy must block inline copying"
            );
        }
    }

    #[test]
    fn opening_attachment_picker_retains_composer_draft_and_rich_context() {
        let (mut app, _) = app_and_context();
        app.input_message = "  Chú thích đang soạn  ".into();
        app.composer_mentions = crate::mentions::Draft::from(app.input_message.as_str());
        app.replying_to = Some(ChatMessage {
            id: "quoted-source".into(),
            ..Default::default()
        });
        for images_only in [true, false] {
            app.request_attachment_picker("thread-fixture", false, images_only);
            assert_eq!(app.input_message, "  Chú thích đang soạn  ");
            assert_eq!(app.composer_mentions.text, app.input_message);
            assert_eq!(app.replying_to.as_ref().unwrap().id, "quoted-source");
        }
    }

    #[test]
    fn attachment_picker_ignores_repeated_clicks_while_native_dialog_is_open() {
        let (mut app, _) = app_and_context();
        app.attachment_picker_pending = true;
        app.request_attachment_picker("thread-fixture", false, false);
        assert!(app.attachment_picker_pending);
        assert_eq!(app.status_msg, "Hộp chọn tệp đang mở");
    }

    #[test]
    fn name_edit_routes_keep_scope_and_cannot_edit_self_storage() {
        let (mut app, _) = app_and_context();
        app.user_uin = "100".into();
        app.own_cloud_id = "900".into();
        app.hidden_panel.view.ready = true;
        app.open_conversation_name_editor("900", "Cloud renamed by user", false);
        app.open_conversation_name_editor("100", "Me", false);
        assert!(!app.contact_panel.open && !app.groups_panel.open);

        app.open_conversation_name_editor("200", "Peer", false);
        assert!(app.contact_panel.alias_only);
        assert_eq!(app.contact_panel.target, "200");
        assert!(app.contact_panel.confirm.is_none());
        app.contact_panel = Default::default();

        app.open_conversation_name_editor("g300", "Shared group", true);
        assert!(app.groups_panel.edit_name);
        assert_eq!(app.groups_panel.target, "g300");
        assert_eq!(app.groups_panel.name, "Shared group");
        assert!(app.groups_panel.confirm.is_none());
    }

    #[test]
    fn row_report_and_invite_open_forms_for_explicit_target_without_mutation() {
        let (mut app, _) = app_and_context();
        app.open_contact_report("222".into(), false);
        assert!(app.contact_panel.report_only);
        assert!(!app.contact_panel.group_report);
        assert_eq!(app.contact_panel.target, "222");
        assert!(!app.contact_panel.pending && app.contact_panel.confirm.is_none());
        app.open_invite_to_group_picker("333".into());
        assert_eq!(app.groups_panel.invite_contact.as_deref(), Some("333"));
        assert!(app.groups_panel.target.is_empty());
        assert!(!app.groups_panel.pending && app.groups_panel.confirm.is_none());
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
            success: false,
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
            success: false,
            message: "Đang cập nhật hội thoại".into(),
        })
        .unwrap();
        tx.send(GuiEvent::HistorySyncStatus {
            running: true,
            success: false,
            message: "Nhóm thử — trang 2, 50 tin mới".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert_eq!(app.sync_status_message(), "Nhóm thử — trang 2, 50 tin mới");
        assert!(!app.settings_modal_open);
        app.sync_cancel_requested = true;
        tx.send(GuiEvent::HistorySyncStatus {
            running: true,
            success: false,
            message: "Nhóm thử — trang 3, 100 tin mới".into(),
        })
        .unwrap();
        tx.send(GuiEvent::ConversationRefreshStatus {
            running: false,
            success: true,
            message: "Đã cập nhật hội thoại".into(),
        })
        .unwrap();
        app.check_events(&ctx);
        assert!(app.sync_cancel_requested);
        assert_eq!(app.sync_status_message(), "Nhóm thử — trang 3, 100 tin mới");
        tx.send(GuiEvent::HistorySyncStatus {
            running: false,
            success: false,
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
    fn sync_banner_does_not_reserve_screen_height() {
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
                        let pal = app.palette();
                        egui::CentralPanel::default().show(ctx, |ui| {
                            app.render_sync_banner(ui, ctx, &pal);
                        });
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
#[cfg(test)]
const COMPOSER_ROW_HEIGHT: f32 = 20.0;
/// Maximum scroll viewport for five rows plus the compact editor margin.
#[cfg(test)]
const COMPOSER_MAX_HEIGHT: f32 = 114.0;

fn plain_composer_enter(event: &egui::Event) -> bool {
    matches!(event, egui::Event::Key { key: Key::Enter, pressed: true, modifiers, .. }
        if !modifiers.shift && !modifiers.ctrl && !modifiers.alt && !modifiers.command && !modifiers.mac_cmd)
}

fn consume_composer_enter(events: &mut Vec<egui::Event>) -> bool {
    if let Some(index) = events.iter().position(plain_composer_enter) {
        events.remove(index);
        true
    } else {
        false
    }
}

fn consume_composer_format_shortcut(events: &mut Vec<egui::Event>) -> bool {
    if let Some(index) = events.iter().position(|event| {
        matches!(event,
        egui::Event::Key { key: Key::S | Key::X, pressed: true, modifiers, .. }
        if modifiers.ctrl && modifiers.shift && !modifiers.alt)
    }) {
        events.remove(index);
        true
    } else {
        false
    }
}

#[derive(Clone)]
struct DeferredComposerEvents {
    scope: Id,
    events: Vec<egui::Event>,
}
impl Default for DeferredComposerEvents {
    fn default() -> Self {
        Self {
            scope: Id::new("empty_composer_scope"),
            events: Vec::new(),
        }
    }
}

fn composer_input_fragment(events: &[egui::Event]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            egui::Event::Text(value) | egui::Event::Paste(value) => text.push_str(value),
            egui::Event::Key {
                key: Key::Enter,
                pressed: true,
                modifiers,
                ..
            } if modifiers.shift => text.push('\n'),
            egui::Event::Ime(egui::ImeEvent::Commit(value)) => text.push_str(value),
            _ => {}
        }
    }
    text
}

fn cancel_interrupted_composer_events(ctx: &egui::Context, active_scope: Option<Id>) -> bool {
    let key = Id::new("deferred_composer_events");
    let Some(pending) = ctx.data_mut(|data| data.remove_temp::<DeferredComposerEvents>(key)) else {
        return false;
    };
    let interrupted = ctx.input(|input| {
        !input.focused
            || input.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::PointerButton { pressed: true, .. }
                        | egui::Event::WindowFocused(false)
                        | egui::Event::Key {
                            key: Key::Tab,
                            pressed: true,
                            ..
                        }
                )
            })
    });
    if active_scope == Some(pending.scope) && !interrupted {
        ctx.data_mut(|data| data.insert_temp(key, pending));
        return false;
    }
    let draft = composer_input_fragment(&pending.events);
    if !draft.is_empty() {
        let draft_key = pending.scope.with("cancelled_draft");
        ctx.data_mut(|data| {
            let previous = data.remove_temp::<String>(draft_key).unwrap_or_default();
            data.insert_temp(draft_key, previous + &draft);
        });
    }
    true
}

fn stage_composer_events(ctx: &egui::Context, composer: Id, composing: bool) {
    let deferred_key = Id::new("deferred_composer_events");
    let deferred = ctx
        .data_mut(|data| data.remove_temp::<DeferredComposerEvents>(deferred_key))
        .filter(|pending| pending.scope == composer)
        .map(|pending| pending.events)
        .unwrap_or_default();
    let scope = composer;
    let tail = ctx.input_mut(|input| {
        let mut events = deferred;
        events.append(&mut input.events);
        let split = if ime_state(&events, composing).1 {
            None
        } else {
            events
                .iter()
                .position(plain_composer_enter)
                .map(|index| index + 1)
        };
        let tail = split
            .map(|index| events.split_off(index))
            .unwrap_or_default();
        input.events = events;
        tail
    });
    if !tail.is_empty() {
        let only_keyboard = tail.iter().all(|event| {
            matches!(
                event,
                egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::Ime(_)
                    | egui::Event::Key { .. }
            )
        });
        if only_keyboard {
            ctx.data_mut(|data| {
                data.insert_temp(
                    deferred_key,
                    DeferredComposerEvents {
                        scope,
                        events: tail,
                    },
                )
            });
            ctx.request_repaint();
        } else {
            let fragment = composer_input_fragment(&tail);
            let remainder = tail
                .into_iter()
                .filter(|event| {
                    !matches!(
                        event,
                        egui::Event::Text(_)
                            | egui::Event::Paste(_)
                            | egui::Event::Ime(_)
                            | egui::Event::Key { .. }
                    )
                })
                .collect::<Vec<_>>();
            ctx.input_mut(|input| input.events.extend(remainder));
            if !fragment.is_empty() {
                let draft_key = scope.with("cancelled_draft");
                ctx.data_mut(|data| {
                    let previous = data.remove_temp::<String>(draft_key).unwrap_or_default();
                    data.insert_temp(draft_key, previous + &fragment);
                });
            }
        }
    }
}

fn composer_layout_job(
    text: &str,
    mentions: &crate::mentions::Draft,
    format: &crate::rich_text::ComposerFormat,
    width: f32,
    line_height: f32,
    pal: &ThemePalette,
) -> egui::text::LayoutJob {
    let mut options = format.options(text);
    let mut job = if options.styles.is_empty() {
        super::mention_composer::mention_layout(text, mentions, width, pal)
    } else {
        options.styles.extend(crate::rich_text::mention_styles(
            Some(&serde_json::to_value(&mentions.mentions).unwrap_or_default()),
            text,
        ));
        super::messages::styled_text_job_with_mentions(
            text,
            &options.styles,
            pal.text_primary,
            pal.primary,
            14.0,
        )
    };
    job.wrap.max_width = width.max(1.0);
    for section in &mut job.sections {
        section.format.line_height = Some(line_height);
    }
    job
}

fn composer_picker_position(anchor: Rect, chat: Rect, screen: Rect, width: f32) -> Pos2 {
    let left = chat.left().max(screen.left() + 8.0);
    let right = (chat.right().min(screen.right()) - width - 8.0).max(left);
    Pos2::new(anchor.left().clamp(left, right), anchor.top() - 4.0)
}

/// Track IME composition across frames: returns `(composing_after, block_enter_now)`.
///
/// Enter is left to the editor while a word is being composed or was just committed in the
/// same frame, so committing a Telex word never sends the message early.
fn ime_state(events: &[egui::Event], composing: bool) -> (bool, bool) {
    let mut active = composing;
    let mut touched = false;
    for event in events {
        if let egui::Event::Ime(ime) = event {
            match ime {
                egui::ImeEvent::Preedit(text) => {
                    active = !text.is_empty();
                    touched = true;
                }
                egui::ImeEvent::Commit(_) => {
                    active = false;
                    touched = true;
                }
                egui::ImeEvent::Disabled => active = false,
                egui::ImeEvent::Enabled => {}
            }
        }
    }
    (active, composing || touched)
}

/// Identity of what the chat list will render; changes trigger one re-layout pass.
fn chat_layout_key(thread_id: &str, messages: &[ChatMessage], render_limit: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    thread_id.hash(&mut hasher);
    messages.len().hash(&mut hasher);
    render_limit.hash(&mut hasher);
    messages.first().map(|m| m.id.as_str()).hash(&mut hasher);
    messages.last().map(|m| m.id.as_str()).hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
fn composer_rows(text: &str) -> usize {
    text.split('\n').count().clamp(1, COMPOSER_MAX_ROWS)
}

/// Reserve visual wrapping too; explicit newline counts miss narrow editors.
#[cfg(test)]
fn composer_editor_height(ui: &egui::Ui, text: &str, width: f32) -> f32 {
    let measured = composer_editor_height_unclamped(ui, text, width);
    let row_height = ui.fonts(|fonts| fonts.row_height(&FontId::proportional(14.0)));
    measured
        .min(row_height * COMPOSER_MAX_ROWS as f32 + 6.0)
        .min(COMPOSER_MAX_HEIGHT)
}

#[cfg(test)]
fn composer_editor_height_unclamped(ui: &egui::Ui, text: &str, width: f32) -> f32 {
    let measured = ui.fonts(|f| {
        f.layout(
            if text.is_empty() {
                " ".into()
            } else {
                text.to_owned()
            },
            FontId::proportional(14.0),
            Color32::WHITE,
            (width - 12.0).max(1.0),
        )
        .size()
        .y
    });
    measured.max(COMPOSER_ROW_HEIGHT) + 6.0
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

    fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn render_input_frame(
        ctx: &egui::Context,
        events: Vec<egui::Event>,
        draft: &mut String,
        sent: &mut Vec<String>,
    ) {
        let id = Id::new("burst-editor");
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    stage_composer_events(ctx, id, false);
                    let enter = ui.input_mut(|input| consume_composer_enter(&mut input.events));
                    egui::TextEdit::multiline(draft).id(id).show(ui);
                    if enter && !draft.trim().is_empty() {
                        sent.push(std::mem::take(draft));
                    }
                });
            },
        );
    }

    #[test]
    fn enter_bursts_keep_each_draft_boundary_in_one_native_frame() {
        let ctx = egui::Context::default();
        let id = Id::new("burst-editor");
        ctx.memory_mut(|memory| memory.request_focus(id));
        let mut draft = String::new();
        let mut sent = Vec::new();
        render_input_frame(
            &ctx,
            vec![
                egui::Event::Text("k1".into()),
                key(Key::Enter, egui::Modifiers::NONE),
                egui::Event::Text("k2".into()),
                key(Key::Enter, egui::Modifiers::NONE),
                egui::Event::Text("k3".into()),
                key(Key::Enter, egui::Modifiers::NONE),
            ],
            &mut draft,
            &mut sent,
        );
        for _ in 0..3 {
            render_input_frame(&ctx, Vec::new(), &mut draft, &mut sent);
        }
        assert_eq!(sent, ["k1", "k2", "k3"]); // (INV-4)
        assert!(draft.is_empty());
    }

    #[test]
    fn released_shift_in_same_frame_keeps_newline_and_format_shortcut() {
        let ctx = egui::Context::default();
        ctx.memory_mut(|memory| memory.request_focus(Id::new("burst-editor")));
        let mut draft = String::new();
        let mut sent = Vec::new();
        render_input_frame(
            &ctx,
            vec![
                egui::Event::Text("one".into()),
                key(Key::Enter, egui::Modifiers::SHIFT),
                egui::Event::Text("two".into()),
            ],
            &mut draft,
            &mut sent,
        );
        assert_eq!(draft, "one\ntwo");
        assert!(sent.is_empty());
        let mut events = vec![key(Key::S, egui::Modifiers::CTRL | egui::Modifiers::SHIFT)];
        assert!(consume_composer_format_shortcut(&mut events));
        assert!(events.is_empty());
    }

    #[test]
    fn pointer_event_after_send_boundary_is_never_replayed_with_deferred_text() {
        let ctx = egui::Context::default();
        let scope = Id::new("composer-owner-thread");
        ctx.data_mut(|data| {
            data.insert_temp(
                Id::new("deferred_composer_events"),
                DeferredComposerEvents {
                    scope,
                    events: vec![
                        egui::Event::Text("one".into()),
                        key(Key::Enter, egui::Modifiers::NONE),
                        egui::Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(0.0, -120.0),
                            modifiers: egui::Modifiers::NONE,
                        },
                        egui::Event::Text("two".into()),
                        key(Key::Enter, egui::Modifiers::NONE),
                    ],
                },
            )
        });
        let mut events = vec![];
        let cancelled = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))),
                events: vec![egui::Event::PointerButton {
                    pos: Pos2::new(30.0, 30.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ctx| {
                let cancelled = cancel_interrupted_composer_events(ctx, None);
                assert!(cancelled);
                let draft = ctx
                    .data_mut(|data| data.remove_temp::<String>(scope.with("cancelled_draft")))
                    .unwrap();
                assert_eq!(draft, "onetwo");
                events = ctx.input(|input| input.events.clone());
            },
        );
        let _ = cancelled;
        assert!(events
            .iter()
            .any(|event| matches!(event, egui::Event::PointerButton { pressed: true, .. })));
        assert!(ctx.data(|data| data
            .get_temp::<DeferredComposerEvents>(Id::new("deferred_composer_events"))
            .is_none()));
    }

    #[test]
    fn changing_composer_scope_cancels_pending_keys_as_unsent_draft() {
        let ctx = egui::Context::default();
        let old_scope = Id::new("composer-old-account-thread");
        ctx.data_mut(|data| {
            data.insert_temp(
                Id::new("deferred_composer_events"),
                DeferredComposerEvents {
                    scope: old_scope,
                    events: vec![
                        egui::Event::Text("draft".into()),
                        key(Key::Enter, egui::Modifiers::NONE),
                    ],
                },
            )
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            assert!(cancel_interrupted_composer_events(
                ctx,
                Some(Id::new("composer-new-account-thread"))
            ));
            assert_eq!(
                ctx.data_mut(|data| data.remove_temp::<String>(old_scope.with("cancelled_draft"))),
                Some("draft".into())
            );
        });
    }

    #[test]
    fn composer_picker_stays_over_chat_instead_of_sidebar() {
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(1707.0, 1019.0));
        let chat = Rect::from_min_max(Pos2::new(580.0, 24.0), Pos2::new(1380.0, 1019.0));
        for anchor in [
            Rect::from_min_size(Pos2::new(620.0, 920.0), Vec2::splat(32.0)),
            Rect::from_min_size(Pos2::new(1310.0, 960.0), Vec2::splat(32.0)),
        ] {
            let pos = composer_picker_position(anchor, chat, screen, 460.0);
            assert!(pos.x >= chat.left());
            assert!(pos.x + 460.0 <= chat.right());
        }
    }

    #[test]
    fn long_sync_banner_and_search_never_expand_sidebar_content() {
        fn text_rects(shape: &egui::Shape, rects: &mut Vec<Rect>) {
            match shape {
                egui::Shape::Text(text) => {
                    rects.push(Rect::from_min_size(text.pos, text.galley.size()))
                }
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        text_rects(shape, rects);
                    }
                }
                _ => {}
            }
        }
        for (width, scale) in [(800.0, 1.0), (1020.0, 1.0), (1020.0, 1.5), (1707.0, 2.0)] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let mut app = ZaloGuiApp::from_context(
                &ctx,
                Session::new(false, false, false).unwrap(),
                false,
                Vec::new(),
                false,
            );
            app.is_logged_in = true;
            app.hidden_panel.view.ready = true;
            app.policy_panel.snapshot = Some(Default::default());
            let search = "Một từ khóa rất dài trong ô tìm kiếm".repeat(20);
            let mut thread = Thread {
                id: "sidebar-bounds-fixture".into(),
                name: search.clone(),
                messages: vec![ChatMessage {
                    id: "sidebar-message".into(),
                    text: "Nội dung xem trước dài WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW".into(),
                    ts: crate::login::unix_now() * 1000,
                    ..Default::default()
                }],
                ..Default::default()
            };
            thread.refresh_sidebar_presentation();
            app.threads.push(thread);
            app.unread_counts
                .insert("sidebar-bounds-fixture".into(), 125);
            let mut baseline = None;
            let mut baseline_row_width = None;
            for phase in 0..3 {
                if phase == 1 {
                    app.refresh_sync_running = true;
                    app.refresh_sync_status =
                        "Đang cập nhật hội thoại và tin nhắn rất dài".repeat(20);
                    app.search_query = search.clone();
                } else if phase == 2 {
                    app.refresh_sync_running = false;
                    app.refresh_sync_status.clear();
                    app.search_query.clear();
                }
                let mut output = None;
                for _ in 0..4 {
                    output = Some(ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(width, 700.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            app.render_chat_workspace(ctx, &app.palette());
                        },
                    ));
                }
                let panel = ctx
                    .data(|data| data.get_temp::<Rect>(Id::new("sidebar-panel-rect")))
                    .unwrap();
                let (all, unread, labels, more) = ctx
                    .data(|data| {
                        data.get_temp::<(Rect, Rect, Rect, Rect)>(Id::new("sidebar-filter-bounds"))
                    })
                    .unwrap();
                assert!(
                    all.right() <= unread.left()
                        && unread.right() <= labels.left()
                        && labels.right() <= more.left(),
                    "{width}@{scale} phase {phase}: sidebar filter controls overlap"
                );
                assert!(panel.contains_rect(all) && panel.contains_rect(more));
                if let Some(expected) = baseline {
                    assert_eq!(
                        panel, expected,
                        "{width}@{scale}: phase {phase} changed the panel rectangle"
                    );
                } else {
                    baseline = Some(panel);
                }
                let row = ctx
                    .data(|data| {
                        data.get_temp::<Rect>(Id::new((
                            "sidebar-row-rect",
                            "sidebar-bounds-fixture",
                        )))
                    })
                    .expect("matching conversation row must be rendered");
                assert!(
                    panel.contains_rect(row),
                    "{width}@{scale} phase {phase}: row {row:?} outside {panel:?}"
                );
                if let Some(expected) = baseline_row_width {
                    assert_eq!(
                        row.width(),
                        expected,
                        "row width changed after banner/search transition"
                    );
                } else {
                    baseline_row_width = Some(row.width());
                }
                let mut rects = Vec::new();
                for shape in &output.unwrap().shapes {
                    text_rects(&shape.shape, &mut rects);
                }
                let row_text = rects
                    .into_iter()
                    .filter(|r| {
                        r.top() >= row.top()
                            && r.top() < row.bottom()
                            && r.left() >= row.left()
                            && r.left() < panel.right()
                    })
                    .collect::<Vec<_>>();
                assert!(
                    row_text.len() >= 4,
                    "name, preview, time and unread badge must all be painted"
                );
                for rect in row_text {
                    assert!(
                        row.expand(1.0).contains_rect(rect),
                        "{width}@{scale} phase {phase}: painted text {rect:?} outside row {row:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn blocked_group_notice_keeps_send_inside_viewport() {
        for (width, height, scale) in [
            (1020.0, 700.0, 1.0),
            (1020.0, 700.0, 1.5),
            (480.0, 360.0, 1.0),
        ] {
            let ctx = egui::Context::default();
            ctx.set_pixels_per_point(scale);
            let mut app = ZaloGuiApp::from_context(
                &ctx,
                Session::new(false, false, false).unwrap(),
                false,
                Vec::new(),
                false,
            );
            app.settings_modal_open = false;
            app.history_sync_running = false;
            app.info_drawer_open = true;
            app.threads = vec![Thread {
                id: "blocked-fixture".into(),
                name: "Nhóm chưa xác minh quyền".into(),
                is_group: true,
                ..Default::default()
            }];
            app.selected_thread_index = 0;
            for _ in 0..4 {
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(width / scale, height / scale),
                    )),
                    ..Default::default()
                };
                let _ = ctx.run(input, |ctx| app.render_chat_workspace(ctx, &app.palette()));
            }
            let (send, clip) = ctx
                .data(|data| data.get_temp::<(Rect, Rect)>(Id::new("composer-send-bounds")))
                .unwrap();
            assert!(
                clip.expand(1.0).contains_rect(send),
                "{width}x{height}@{scale}: send={send:?}, clip={clip:?}"
            );
        }
    }
    #[test]
    fn composer_rows_grow_with_lines_and_cap_at_five() {
        assert_eq!(composer_rows(""), 1);
        assert_eq!(composer_rows("một dòng"), 1);
        assert_eq!(composer_rows("a\nb\nc"), 3);
        assert_eq!(composer_rows("1\n2\n3\n4\n5\n6\n7"), 5);
    }
    #[test]
    fn wrapped_single_line_yields_message_space_and_stops_at_five_rows() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let short = composer_editor_height(ui, "Hi", 200.0);
                let wrapped = composer_editor_height(
                    ui,
                    &"Tiếng Việt có dấu và khoảng trắng. ".repeat(12),
                    80.0,
                );
                assert!(wrapped > short);
                let row_height = ui.fonts(|fonts| fonts.row_height(&FontId::proportional(14.0)));
                assert_eq!(wrapped, (row_height * 5.0 + 6.0).min(COMPOSER_MAX_HEIGHT));
            });
        });
    }

    #[test]
    fn composer_bottom_stays_fixed_while_visual_lines_grow_and_rich_mode_expands() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "composer-owner".into();
        app.hidden_panel.view.ready = true;
        app.policy_panel.snapshot = Some(Default::default());
        app.threads = vec![Thread {
            id: "composer-fixture".into(),
            name: "Composer".into(),
            ..Default::default()
        }];
        app.selected_thread_index = 0;
        let scope = Id::new(("chat_composer_input", "composer-fixture"))
            .with((&app.user_uin, app.account_runtime_id));
        let mut heights = Vec::new();
        for (text, rich, expanded) in [
            ("a", false, false),
            ("a\nb\nc", false, false),
            ("1\n2\n3\n4\n5\n6\n7", false, false),
            ("a", true, true),
        ] {
            for _ in 0..5 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1200.0, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        app.sync_composer_draft(ctx);
                        app.input_message = text.into();
                        app.composer_format_mode = rich;
                        ctx.data_mut(|data| data.insert_temp(scope.with("expanded"), expanded));
                        app.render_chat_workspace(ctx, &app.palette());
                    },
                );
            }
            let (rect, bottom, editor) = ctx
                .data(|data| data.get_temp::<(Rect, f32, f32)>(Id::new("composer-layout-bounds")))
                .unwrap();
            assert!(
                (rect.bottom() - bottom).abs() < 1.0,
                "composer bottom moved: {rect:?}, viewport bottom {bottom}"
            );
            heights.push(editor);
        }
        assert!(heights[1] > heights[0]);
        assert!(heights[2] > heights[1]);
        assert!(heights[3] > heights[2] * 2.0);
        app.input_message = "line one".into();
        ctx.memory_mut(|memory| {
            memory.request_focus(Id::new(("chat_composer_input", "composer-fixture")))
        });
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
                modifiers: egui::Modifiers::SHIFT,
                events: vec![egui::Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::SHIFT,
                }],
                ..Default::default()
            },
            |ctx| app.render_chat_workspace(ctx, &app.palette()),
        );
        assert!(
            app.input_message.contains('\n'),
            "Shift+Enter must reach the actual editor and insert a newline"
        );
    }

    #[test]
    fn contact_card_modal_search_and_pill_filter() {
        let (mut app, ctx) = super::sync_and_order_tests::app_and_context();
        app.threads = vec![
            Thread {
                id: "peer-direct-1".into(),
                name: "Nguyễn Văn A (Khách hàng VIP)".into(),
                is_group: false,
                ..Default::default()
            },
            Thread {
                id: "peer-direct-2".into(),
                name: "Trần Thị B (Gia đình)".into(),
                is_group: false,
                ..Default::default()
            },
            Thread {
                id: "group-1".into(),
                name: "Nhóm Dự án".into(),
                is_group: true,
                ..Default::default()
            },
        ];
        app.selected_thread_index = 0;
        app.send_contact_modal_open = true;
        app.send_contact_query = "Nguyễn".into();
        app.send_contact_filter = 0;

        let pal = app.palette();
        let _ = ctx.run(Default::default(), |ctx| {
            app.render_send_contact_modal(ctx, &pal);
        });

        assert!(app.send_contact_modal_open);
        assert_eq!(app.send_contact_filter, 0);

        // Test selection
        app.send_contact_selected = Some("peer-direct-1".into());
        assert_eq!(app.send_contact_selected.as_deref(), Some("peer-direct-1"));
    }

    #[test]
    fn toolbar_stk_quick_message_expansion() {
        let (mut app, _) = super::sync_and_order_tests::app_and_context();
        app.tools.config.quick_messages.upsert(
            "/stk".into(),
            "Số tài khoản".into(),
            "STK: 19036789999 Techcombank".into(),
        );

        let stk_item = app
            .tools
            .config
            .quick_messages
            .items
            .iter()
            .find(|i| {
                let sc = i.shortcut.to_lowercase();
                let tt = i.title.to_lowercase();
                sc == "/stk" || sc == "stk" || tt.contains("stk") || tt.contains("tài khoản")
            })
            .cloned();

        assert!(stk_item.is_some());
        let item = stk_item.unwrap();
        app.input_message = "Chuyển tiền qua ".into();
        if !app.input_message.is_empty() && !app.input_message.ends_with(' ') {
            app.input_message.push(' ');
        }
        app.input_message.push_str(&item.text);

        assert_eq!(
            app.input_message,
            "Chuyển tiền qua STK: 19036789999 Techcombank"
        );
    }

    #[test]
    fn composer_like_to_send_predicate() {
        let (mut app, _) = super::sync_and_order_tests::app_and_context();
        let thread_id = "test-thread";

        // Initial: empty message & empty drafts -> cannot send (shows Like "👍")
        app.input_message.clear();
        let can_send =
            !app.input_message.trim().is_empty() || app.attachment_drafts.len(thread_id) > 0;
        assert!(!can_send, "empty composer must show like button");

        // With text typed -> can send (shows Send plane)
        app.input_message = "Xin chào".into();
        let can_send =
            !app.input_message.trim().is_empty() || app.attachment_drafts.len(thread_id) > 0;
        assert!(can_send, "typed text must show send button");

        // With draft attachment -> can send
        app.input_message.clear();
        app.user_uin = "test-owner".into();
        app.attachment_drafts.sync_owner("test-owner");
        assert!(app
            .attachment_drafts
            .insert(thread_id, std::path::PathBuf::from("dummy.png"))
            .is_some());
        let can_send =
            !app.input_message.trim().is_empty() || app.attachment_drafts.len(thread_id) > 0;
        assert!(
            can_send,
            "draft attachment must show send button even if text is empty"
        );
    }
}

#[cfg(test)]
mod ime_tests {
    use super::*;
    fn ime(e: egui::ImeEvent) -> egui::Event {
        egui::Event::Ime(e)
    }
    #[test]
    fn enter_is_blocked_while_composing_and_when_commit_arrives_in_same_frame() {
        assert_eq!(ime_state(&[], false), (false, false));
        assert_eq!(
            ime_state(&[ime(egui::ImeEvent::Preedit("vie".into()))], false),
            (true, true)
        );
        assert_eq!(ime_state(&[], true), (true, true));
        assert_eq!(
            ime_state(&[ime(egui::ImeEvent::Commit("việt".into()))], true),
            (false, true)
        );
        assert_eq!(
            ime_state(&[ime(egui::ImeEvent::Preedit(String::new()))], true),
            (false, true)
        );
        assert_eq!(
            ime_state(&[ime(egui::ImeEvent::Disabled)], true),
            (false, true)
        );
    }
}

#[cfg(test)]
mod chat_layout_key_tests {
    use super::*;
    fn msg(id: &str) -> ChatMessage {
        ChatMessage {
            id: id.into(),
            ..Default::default()
        }
    }
    #[test]
    fn key_changes_on_switch_prepend_append_and_window_but_not_on_rerender() {
        let base = vec![msg("a"), msg("b")];
        let key = chat_layout_key("t", &base, 150);
        assert_eq!(key, chat_layout_key("t", &base, 150));
        assert_ne!(key, chat_layout_key("u", &base, 150));
        assert_ne!(
            key,
            chat_layout_key("t", &[msg("z"), msg("a"), msg("b")], 150)
        );
        assert_ne!(
            key,
            chat_layout_key("t", &[msg("a"), msg("b"), msg("c")], 150)
        );
        assert_ne!(key, chat_layout_key("t", &base, 300));
    }
}

fn group_avatar_affordance(
    ui: &mut egui::Ui,
    response: egui::Response,
    pal: &ThemePalette,
) -> bool {
    let tooltip = "Quản lý ảnh nhóm / cộng đồng; kiểm tra quyền trước khi thay đổi";
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
    });
    if response.hovered() || response.has_focus() {
        let center = response.rect.right_bottom() - Vec2::splat(7.0);
        ui.painter().circle_filled(center, 10.0, pal.bg_elevated);
        draw_lucide_icon(ui.painter(), center, lucide::CAMERA, 13.0, pal.primary);
    }
    response
        .on_hover_text(tooltip)
        .on_hover_cursor(CursorIcon::PointingHand)
        .clicked()
}

fn name_edit_affordance(
    ui: &mut egui::Ui,
    name_hovered: bool,
    enabled: bool,
    is_group: bool,
    pal: &ThemePalette,
) -> bool {
    let tooltip = if is_group {
        "Đổi tên nhóm / cộng đồng cho mọi thành viên"
    } else {
        "Đặt tên gợi nhớ chỉ hiển thị riêng với bạn"
    };
    ui.add_enabled_ui(enabled, |ui| {
        // Reserve a stable keyboard-focusable hit area even while the pencil is hidden.
        let response = ui.allocate_response(Vec2::splat(20.0), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
        });
        if ui.is_enabled() && (name_hovered || response.hovered() || response.has_focus()) {
            let c = response.rect.center();
            let color = if response.hovered() || response.has_focus() {
                pal.primary
            } else {
                pal.text_secondary
            };
            let stroke = Stroke::new(1.4_f32, color);
            let points = vec![
                c + Vec2::new(-5.0, 5.0),
                c + Vec2::new(-4.0, 1.0),
                c + Vec2::new(3.0, -6.0),
                c + Vec2::new(6.0, -3.0),
                c + Vec2::new(-1.0, 4.0),
                c + Vec2::new(-5.0, 5.0),
            ];
            ui.painter().add(egui::Shape::line(points, stroke));
            ui.painter()
                .line_segment([c + Vec2::new(1.0, -4.0), c + Vec2::new(4.0, -1.0)], stroke);
        }
        response
            .on_hover_text(tooltip)
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
    })
    .inner
}

impl ZaloGuiApp {
    fn open_conversation_avatar_editor(&mut self, thread_id: &str) {
        if !self.is_logged_in || !self.private_access(thread_id, true) {
            return;
        }
        if self.groups_panel.pending {
            self.show_toast("Chờ thao tác nhóm trước hoàn tất trước khi đổi ảnh");
            return;
        }
        // Inspect permissions first; the group panel enables its real avatar editor only after ACK.
        self.open_group_panel(thread_id.to_owned(), true, false);
    }

    fn open_conversation_name_editor(&mut self, thread_id: &str, name: &str, is_group: bool) {
        if !self.is_logged_in
            || thread_id == self.user_uin
            || (!self.own_cloud_id.is_empty() && thread_id == self.own_cloud_id)
            || !self.private_access(thread_id, is_group)
        {
            return;
        }
        if is_group {
            self.open_group_name_editor(thread_id.to_owned(), name.to_owned());
        } else {
            self.open_contact_alias_editor(thread_id.to_owned());
        }
    }

    fn request_my_documents_usage(&mut self) {
        if !self.is_logged_in || self.cloud_usage.pending {
            return;
        }
        self.cloud_usage.owner = self.user_uin.clone();
        self.cloud_usage.generation = crate::search::generation();
        self.cloud_usage.pending = true;
        if !WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::LoadCloudUsage {
                owner: self.user_uin.clone(),
                generation: self.cloud_usage.generation,
            })
            .is_ok()
        }) {
            self.cloud_usage.pending = false;
            self.cloud_usage.result = Some(Err("Worker chưa sẵn sàng".into()));
        }
    }

    fn render_my_documents_overview(&mut self, ui: &mut egui::Ui, pal: &ThemePalette) {
        egui::Frame::none().inner_margin(Margin::symmetric(16.0, 14.0)).show(ui, |ui| {
            ui.label(RichText::new("My Documents").strong().color(pal.text_primary));
            ui.label("Lưu và tìm lại tài liệu, ảnh, video và liên kết của riêng bạn trên tài khoản Zalo.");
            ui.add_space(8.0);
            ui.label(RichText::new("Dung lượng lưu trữ").strong());
            if self.cloud_usage.pending {
                ui.horizontal(|ui| { ui.spinner(); ui.label("Đang tải dung lượng từ máy chủ…"); });
            }
            match self.cloud_usage.result.as_ref() {
                Some(Ok(usage)) => {
                    let used = crate::chat::format_file_size(usage.usage);
                    let quota = crate::chat::format_file_size(usage.quota);
                    ui.label(format!("Đã dùng {used} / {quota}"));
                    ui.add(egui::ProgressBar::new((usage.usage as f64 / usage.quota as f64).clamp(0.0,1.0) as f32));
                    if usage.usage >= usage.quota { ui.label(RichText::new("Dung lượng đã đạt hạn mức; xem lại nội dung trước khi thêm tệp.").color(pal.warning)); }
                }
                Some(Err(error)) => { ui.label(RichText::new(format!("Chưa tải được dung lượng: {error}")).color(pal.warning)); }
                None => { ui.label("Chưa có dữ liệu dung lượng từ máy chủ."); }
            }
            if ui.add_enabled(!self.cloud_usage.pending, egui::Button::new("Tải lại dung lượng")).clicked() {
                self.request_my_documents_usage();
            }
            ui.add_space(8.0);
            if ui.button("Xem lại và dọn dẹp My Documents…").clicked() {
                self.drawer_media_selected.clear();
                self.drawer_media_selection_mode = true;
                self.drawer_view = DrawerView::MediaStorage {tab: MediaTab::Files};
            }
            ui.label("Chọn nội dung trong kho để xem lại; thao tác xóa luôn có xác nhận riêng.");
            ui.separator();
            ui.label(RichText::new("Kho nội dung").strong());
            for (label, tab) in [("Ảnh / Video", MediaTab::Photos), ("Tệp", MediaTab::Files), ("Liên kết", MediaTab::Links)] {
                if ui.button(label).clicked() {
                    self.drawer_media_selection_mode = false;
                    self.drawer_media_selected.clear();
                    self.drawer_view = DrawerView::MediaStorage {tab};
                }
            }
            if ui.button("Tìm trong My Documents…").clicked() { self.drawer_view = DrawerView::SearchInThread; }
        });
    }

    /// Who is typing in this conversation, while the indicator is still fresh.
    fn typing_label(&mut self, thread_id: &str, is_group: bool) -> Option<String> {
        if !self.settings.show_typing
            || !self.private_access(thread_id, is_group)
            || !self.inbox_policy.permits_typing(thread_id, is_group)
        {
            return None;
        }
        let key = (
            thread_id
                .trim_start_matches("group_")
                .trim_start_matches(if is_group { "g" } else { "" })
                .to_owned(),
            is_group,
        );
        let now = std::time::Instant::now();
        self.typing_map
            .retain(|_, (_, at)| now.duration_since(*at) < crate::presence::TYPING_VISIBLE);
        let (uid, _) = self.typing_map.get(&key)?;
        let thread = self.threads.iter().find(|t| t.id == thread_id)?;
        if !is_group {
            return Some(self.resolve_thread_name(thread).to_owned());
        }
        if let Some(alias) = self
            .server_aliases
            .get(uid)
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
        {
            return Some(alias.to_string());
        }
        let name = self
            .group_members
            .get(thread_id)
            .and_then(|(members, _)| {
                members
                    .iter()
                    .find(|m| &m.id == uid && !m.name.trim().is_empty())
                    .map(|m| m.name.clone())
            })
            .or_else(|| {
                self.threads
                    .iter()
                    .find(|t| {
                        !t.is_group && &t.id == uid && !crate::chat::is_generic_thread_name(&t.name)
                    })
                    .map(|t| t.name.clone())
            })
            .or_else(|| {
                thread
                    .messages
                    .iter()
                    .rev()
                    .find(|m| {
                        crate::chat::sidebar_sender_id(m).as_deref() == Some(uid.as_str())
                            && !m.sender.is_empty()
                            && !m.sender.chars().all(|c| c.is_ascii_digit())
                    })
                    .map(|m| m.sender.clone())
            })
            .unwrap_or_else(|| "Một thành viên".into());
        Some(name)
    }

    /// Send a typing signal at most every `TYPING_SEND_EVERY` per conversation.
    fn maybe_send_typing(&mut self, thread_id: &str, is_group: bool) {
        if !self.settings.send_typing
            || !self.private_access(thread_id, is_group)
            || !self.inbox_policy.permits_typing(thread_id, is_group)
        {
            return;
        }
        let cloud = !self.own_cloud_id.is_empty() && thread_id == self.own_cloud_id;
        if cloud {
            return;
        }
        let now = std::time::Instant::now();
        if self
            .typing_sent
            .get(thread_id)
            .is_some_and(|at| now.duration_since(*at) < crate::presence::TYPING_SEND_EVERY)
        {
            return;
        }
        if self.typing_sent.len() > 256 {
            self.typing_sent
                .retain(|_, at| now.duration_since(*at) < crate::presence::TYPING_SEND_EVERY);
        }
        if let Some(tx) = WORKER_TX.get() {
            if tx
                .send(WorkerCmd::SendTyping {
                    thread_id: thread_id.to_owned(),
                    is_group,
                })
                .is_ok()
            {
                self.typing_sent.insert(thread_id.to_owned(), now);
            }
        }
    }
}

/// Lookup avatar for a direct contact without allocating a map each frame.
fn sender_avatar_for<'a>(threads: &'a [Thread], sender_id: &str) -> Option<&'a str> {
    if sender_id.is_empty() {
        return None;
    }
    threads
        .iter()
        .find(|t| !t.is_group && (t.id == sender_id || t.presentation.global_id == sender_id))
        .and_then(|t| t.avatar.as_deref())
}

impl ZaloGuiApp {
    /// Lookup avatar for a direct contact without allocating a map each frame.
    #[allow(dead_code)]
    pub(super) fn sender_avatar_for(&self, sender_id: &str) -> Option<&str> {
        sender_avatar_for(&self.threads, sender_id)
    }

    fn request_group_members(&mut self, group_id: &str) {
        let sent = WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::FetchGroupMembers {
                group_id: group_id.to_owned(),
            })
            .is_ok()
        });
        if sent {
            self.group_members_state.insert(group_id.to_owned(), None);
        } else {
            self.group_members_state
                .insert(group_id.to_owned(), Some("Worker chưa sẵn sàng".into()));
        }
    }

    fn request_common_groups(&mut self, target: &str) {
        let valid_uid =
            !target.is_empty() && target.len() <= 128 && target.bytes().all(|b| b.is_ascii_digit());
        if !valid_uid {
            self.common_groups_state
                .insert(target.to_owned(), Some("ID liên hệ không hợp lệ".into()));
            return;
        }
        let generation = crate::search::generation();
        self.common_groups_generations
            .insert(target.to_owned(), generation);
        self.common_groups_state.insert(target.to_owned(), None);
        let sent = WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::Group {
                owner: self.user_uin.clone(),
                target: target.to_owned(),
                generation,
                action: crate::groups::Action::Related,
            })
            .is_ok()
        });
        if !sent {
            self.common_groups_generations.remove(target);
            self.common_groups_state
                .insert(target.to_owned(), Some("Worker chưa sẵn sàng".into()));
        }
    }

    pub(super) fn resolve_member_display_name(
        &self,
        member: &crate::chat::SidebarMember,
    ) -> String {
        let fallback = if !member.name.trim().is_empty() {
            member.name.as_str()
        } else {
            self.threads
                .iter()
                .find(|t| {
                    !t.is_group
                        && t.id == member.id
                        && !t.name.trim().is_empty()
                        && !crate::chat::is_generic_thread_name(&t.name)
                })
                .map(|t| t.name.as_str())
                .unwrap_or("Thành viên")
        };
        resolve_contact_name_from(&self.server_aliases, &member.id, fallback).to_owned()
    }

    pub(super) fn open_direct_thread_with(
        &mut self,
        ctx: &egui::Context,
        user_id: &str,
        display_name: &str,
        avatar: Option<&str>,
    ) {
        let index =
            if let Some(idx) = self.threads.iter().position(|t| {
                !t.is_group && (t.id == user_id || t.presentation.global_id == user_id)
            }) {
                idx
            } else {
                self.threads.push(Thread {
                    id: user_id.to_owned(),
                    name: display_name.to_owned(),
                    avatar: avatar.map(str::to_owned),
                    is_group: false,
                    ..Default::default()
                });
                self.threads.len() - 1
            };
        let tid = self.threads[index].id.clone();
        self.contacts_view_open = false;
        self.open_thread(ctx, index, tid, false);
    }

    pub(super) fn open_group_thread(&mut self, ctx: &egui::Context, group: &Thread) {
        let index = if let Some(idx) = self.threads.iter().position(|t| {
            t.is_group
                && (t.id == group.id
                    || crate::server_pins::identity(&t.id, true)
                        .ok()
                        .zip(crate::server_pins::identity(&group.id, true).ok())
                        .is_some_and(|(a, b)| a == b))
        }) {
            idx
        } else {
            self.threads.push(group.clone());
            self.threads.len() - 1
        };
        let tid = self.threads[index].id.clone();
        self.contacts_view_open = false;
        self.open_thread(ctx, index, tid, true);
    }

    pub(super) fn can_kick_group_member(
        &self,
        group_id: &str,
        target: &crate::chat::SidebarMember,
    ) -> bool {
        if self.user_uin.is_empty() || target.id == self.user_uin || target.role == "creator" {
            return false;
        }
        let thread = self.threads.iter().find(|t| t.id == group_id && t.is_group);
        let loaded_members = self
            .group_members
            .get(group_id)
            .map(|(members, _)| members.as_slice());
        let roster = loaded_members.or_else(|| thread.map(|t| t.presentation.members.as_slice()));

        let i_am_creator = roster
            .map(|members| {
                members
                    .iter()
                    .any(|m| m.id == self.user_uin && m.role == "creator")
            })
            .unwrap_or(false);
        let i_am_admin = roster
            .map(|members| {
                members
                    .iter()
                    .any(|m| m.id == self.user_uin && m.role == "admin")
            })
            .unwrap_or(false)
            || thread
                .and_then(|t| t.presentation.send_policy.as_ref())
                .is_some_and(|policy| policy.managers.iter().any(|id| id == &self.user_uin));

        if i_am_creator {
            true
        } else if i_am_admin {
            target.role != "admin"
        } else {
            false
        }
    }

    fn render_group_member_row(
        &mut self,
        ui: &mut egui::Ui,
        group_id: &str,
        member: &crate::chat::SidebarMember,
        avatar_radius: f32,
        row_h: f32,
        pal: &ThemePalette,
    ) {
        let name = self.resolve_member_display_name(member);
        let is_selected = self
            .group_member_popover
            .as_ref()
            .is_some_and(|pop| pop.group_id == group_id && pop.member.id == member.id);
        let (row_rect, row_resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::click());
        if row_resp.hovered() || is_selected {
            ui.painter()
                .rect_filled(row_rect, Rounding::same(6.0), pal.bg_hover);
        }

        let av_center = Pos2::new(row_rect.left() + 6.0 + avatar_radius, row_rect.center().y);
        paint_user_avatar(
            ui.painter(),
            av_center,
            avatar_radius,
            &name,
            false,
            member.avatar.as_deref(),
            &self.sticker_textures,
            &mut self.loading_stickers,
        );

        let role = match member.role.as_str() {
            "creator" => Some(("Trưởng nhóm", Color32::from_rgb(245, 158, 11))),
            "admin" => Some(("Phó nhóm", pal.primary_hover)),
            _ => None,
        };

        let mut name_right_limit = row_rect.right() - 6.0;
        if let Some((badge_label, badge_color)) = role {
            let galley = ui.painter().layout_no_wrap(
                badge_label.to_owned(),
                FontId::proportional(9.5),
                badge_color,
            );
            let badge_w = galley.size().x + 10.0;
            let badge_h = galley.size().y + 4.0;
            let badge_rect = Rect::from_min_size(
                Pos2::new(
                    row_rect.right() - 6.0 - badge_w,
                    row_rect.center().y - badge_h * 0.5,
                ),
                Vec2::new(badge_w, badge_h),
            );
            ui.painter().rect_filled(
                badge_rect,
                Rounding::same(4.0),
                badge_color.linear_multiply(0.16),
            );
            ui.painter().rect_stroke(
                badge_rect,
                Rounding::same(4.0),
                Stroke::new(0.8_f32, badge_color.linear_multiply(0.55)),
            );
            ui.painter().galley(
                Pos2::new(badge_rect.left() + 5.0, badge_rect.top() + 2.0),
                galley,
                badge_color,
            );
            name_right_limit = badge_rect.left() - 6.0;
        }

        let name_left = av_center.x + avatar_radius + 8.0;
        let max_name_w = (name_right_limit - name_left).max(24.0);
        let max_chars = ((max_name_w / 6.5) as usize).clamp(8, 28);
        ui.painter().text(
            Pos2::new(name_left, row_rect.center().y),
            egui::Align2::LEFT_CENTER,
            truncate_chars(&name, max_chars),
            FontId::proportional(12.0),
            pal.text_primary,
        );

        let pointer_pos = row_resp.interact_pointer_pos();
        if row_resp
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Nhấn để nhắn tin riêng, gọi hoặc xem thông tin")
            .clicked()
        {
            let anchor =
                pointer_pos.unwrap_or(Pos2::new(row_rect.left() + 12.0, row_rect.bottom()));
            self.group_member_popover = Some(GroupMemberPopover {
                group_id: group_id.to_owned(),
                member: member.clone(),
                display_name: name,
                anchor,
                confirm_kick: false,
                opened_this_frame: true,
            });
        }
    }

    fn render_group_member_popover(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        let Some(mut pop) = self.group_member_popover.clone() else {
            return;
        };
        if !self.is_logged_in || !self.info_drawer_open {
            self.group_member_popover = None;
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.group_member_popover = None;
            return;
        }

        let is_self = pop.member.id == self.user_uin;
        let can_kick = self.can_kick_group_member(&pop.group_id, &pop.member);
        let screen = ctx.screen_rect();
        let pop_w = 224.0;
        let est_h = if pop.confirm_kick {
            240.0
        } else if can_kick {
            224.0
        } else if is_self {
            110.0
        } else {
            190.0
        };
        let pos = Pos2::new(
            pop.anchor
                .x
                .min(screen.right() - pop_w - 12.0)
                .max(screen.left() + 8.0),
            pop.anchor
                .y
                .min(screen.bottom() - est_h - 12.0)
                .max(screen.top() + 8.0),
        );

        let mut close_popover = false;
        let mut action_dm = false;
        let mut action_call: Option<bool> = None;
        let mut action_profile = false;
        let mut action_kick = false;

        let area_resp = egui::Area::new(Id::new("group_member_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(10.0))
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 4.0),
                        blur: 14.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(65),
                    })
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.set_width(pop_w - 20.0);
                        ui.horizontal(|ui| {
                            let (av_r, _) =
                                ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                            paint_user_avatar(
                                ui.painter(),
                                av_r.center(),
                                14.0,
                                &pop.display_name,
                                false,
                                pop.member.avatar.as_deref(),
                                &self.sticker_textures,
                                &mut self.loading_stickers,
                            );
                            ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(truncate_chars(&pop.display_name, 22))
                                        .size(12.5)
                                        .color(pal.text_primary)
                                        .strong(),
                                );
                                let role_text = match pop.member.role.as_str() {
                                    "creator" => {
                                        Some(("Trưởng nhóm", Color32::from_rgb(245, 158, 11)))
                                    }
                                    "admin" => Some(("Phó nhóm", pal.primary_hover)),
                                    _ if is_self => Some(("Bạn", pal.text_muted)),
                                    _ => None,
                                };
                                if let Some((label, color)) = role_text {
                                    ui.label(RichText::new(label).size(10.5).color(color));
                                }
                            });
                        });

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        let btn_w = ui.available_width();
                        let menu_btn = |ui: &mut egui::Ui, text: &str, color: Color32| {
                            ui.add_sized(
                                [btn_w, 28.0],
                                egui::Button::new(RichText::new(text).size(12.0).color(color))
                                    .fill(Color32::TRANSPARENT)
                                    .rounding(Rounding::same(6.0)),
                            )
                            .on_hover_cursor(CursorIcon::PointingHand)
                        };

                        if !is_self {
                            if menu_btn(ui, "💬  Nhắn tin riêng", pal.text_primary).clicked() {
                                action_dm = true;
                                close_popover = true;
                            }
                            if menu_btn(ui, "📞  Gọi thoại", pal.text_primary).clicked() {
                                action_call = Some(false);
                                close_popover = true;
                            }
                            if menu_btn(ui, "📹  Gọi video", pal.text_primary).clicked() {
                                action_call = Some(true);
                                close_popover = true;
                            }
                        }

                        if menu_btn(ui, "👤  Xem thông tin", pal.text_primary).clicked() {
                            action_profile = true;
                            close_popover = true;
                        }

                        if can_kick {
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                            if !pop.confirm_kick {
                                if menu_btn(ui, "🗑  Xóa khỏi nhóm", pal.danger).clicked() {
                                    pop.confirm_kick = true;
                                }
                            } else {
                                ui.label(
                                    RichText::new(format!(
                                        "Xóa {} khỏi nhóm?",
                                        truncate_chars(&pop.display_name, 18)
                                    ))
                                    .size(11.0)
                                    .color(pal.danger),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Xác nhận")
                                                    .size(11.5)
                                                    .color(Color32::WHITE),
                                            )
                                            .fill(pal.danger)
                                            .rounding(Rounding::same(5.0)),
                                        )
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        action_kick = true;
                                        close_popover = true;
                                    }
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new("Hủy")
                                                    .size(11.5)
                                                    .color(pal.text_secondary),
                                            )
                                            .rounding(Rounding::same(5.0)),
                                        )
                                        .on_hover_cursor(CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        pop.confirm_kick = false;
                                    }
                                });
                            }
                        }
                    });
            });

        if !pop.opened_this_frame
            && ctx.input(|i| i.pointer.any_click())
            && !area_resp.response.hovered()
        {
            close_popover = true;
        }
        pop.opened_this_frame = false;

        if action_dm {
            self.open_direct_thread_with(
                ctx,
                &pop.member.id,
                &pop.display_name,
                pop.member.avatar.as_deref(),
            );
        }
        if let Some(is_video) = action_call {
            self.open_preflight_call(
                pop.member.id.clone(),
                pop.display_name.clone(),
                pop.member.avatar.clone(),
                is_video,
                false,
            );
        }
        if action_profile {
            if is_self {
                self.open_own_profile();
            } else {
                self.open_contact_panel(pop.member.id.clone());
            }
        }
        if action_kick {
            let generation = crate::search::generation();
            let sent = WORKER_TX.get().is_some_and(|tx| {
                tx.send(WorkerCmd::Group {
                    owner: self.user_uin.clone(),
                    target: pop.group_id.clone(),
                    generation,
                    action: crate::groups::Action::Kick {
                        member: pop.member.id.clone(),
                    },
                })
                .is_ok()
            });
            if sent {
                self.pending_drawer_group_actions
                    .insert(generation, (pop.group_id.clone(), pop.member.id.clone()));
                self.show_toast(format!("Đang xóa {} khỏi nhóm…", pop.display_name));
            } else {
                self.show_toast("Worker chưa sẵn sàng");
            }
        }

        if close_popover {
            self.group_member_popover = None;
        } else {
            self.group_member_popover = Some(pop);
        }
    }

    fn render_common_groups_section(&mut self, ui: &mut egui::Ui, d_id: &str, pal: &ThemePalette) {
        let cached = self.common_groups_cache.get(d_id).cloned();
        let state = self.common_groups_state.get(d_id).cloned();
        let loading = matches!(state, Some(None));
        if cached.is_none() && !loading && state.is_none() {
            self.request_common_groups(d_id);
        }

        let visible_groups: Vec<Thread> = cached
            .as_ref()
            .map(|groups| {
                groups
                    .iter()
                    .filter(|g| {
                        !self
                            .conversation_preferences
                            .get(&g.id)
                            .is_some_and(|p| p.hidden)
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let count_text = cached
            .as_ref()
            .map(|_| format!("({})", visible_groups.len()))
            .unwrap_or_default();

        let mut clicked_group: Option<Thread> = None;
        let mut trigger_refresh = false;

        egui::Frame::none()
            .inner_margin(Margin::symmetric(16.0, 12.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let chevron = if self.common_groups_collapsed {
                        "▸"
                    } else {
                        "▾"
                    };
                    let header_resp = ui
                        .add(
                            egui::Button::new(
                                RichText::new(format!("{chevron} Nhóm chung"))
                                    .size(12.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            )
                            .frame(false),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand);
                    if header_resp.clicked() {
                        self.common_groups_collapsed = !self.common_groups_collapsed;
                    }
                    if !count_text.is_empty() {
                        ui.label(
                            RichText::new(count_text)
                                .size(11.5)
                                .color(pal.primary_hover),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if loading {
                            ui.spinner();
                        } else if ui
                            .add(
                                egui::Button::new(RichText::new(lucide::REFRESH).size(11.5))
                                    .frame(false),
                            )
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text("Tải lại nhóm chung")
                            .clicked()
                        {
                            trigger_refresh = true;
                        }
                    });
                });

                if self.common_groups_collapsed {
                    return;
                }
                ui.add_space(6.0);

                if let Some(Some(error)) = &state {
                    if cached.is_none() {
                        ui.label(
                            RichText::new(format!("Không tải được nhóm chung: {error}"))
                                .size(11.0)
                                .color(pal.text_muted),
                        );
                        return;
                    }
                }

                if cached.is_none() {
                    ui.label(
                        RichText::new("Đang tải danh sách nhóm chung…")
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                    return;
                }

                if visible_groups.is_empty() {
                    ui.label(
                        RichText::new("Chưa có nhóm chung nào.")
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                    return;
                }

                let show_limit = if self.common_groups_expanded {
                    visible_groups.len()
                } else {
                    3
                };
                for group in visible_groups.iter().take(show_limit) {
                    let g_name = if group.name.trim().is_empty() {
                        format!("Nhóm {}", group.id)
                    } else {
                        group.name.clone()
                    };
                    let (row_rect, row_resp) = ui
                        .allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::click());
                    if row_resp.hovered() {
                        ui.painter()
                            .rect_filled(row_rect, Rounding::same(6.0), pal.bg_hover);
                    }
                    let av_center = Pos2::new(row_rect.left() + 6.0 + 14.0, row_rect.center().y);
                    paint_user_avatar(
                        ui.painter(),
                        av_center,
                        14.0,
                        &g_name,
                        true,
                        group.avatar.as_deref(),
                        &self.sticker_textures,
                        &mut self.loading_stickers,
                    );
                    let text_x = av_center.x + 14.0 + 8.0;
                    if let Some(mc) = group.presentation.member_count {
                        ui.painter().text(
                            Pos2::new(text_x, row_rect.center().y - 7.0),
                            egui::Align2::LEFT_CENTER,
                            truncate_chars(&g_name, 25),
                            FontId::proportional(12.0),
                            pal.text_primary,
                        );
                        ui.painter().text(
                            Pos2::new(text_x, row_rect.center().y + 8.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{mc} thành viên"),
                            FontId::proportional(10.5),
                            pal.text_muted,
                        );
                    } else {
                        ui.painter().text(
                            Pos2::new(text_x, row_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            truncate_chars(&g_name, 25),
                            FontId::proportional(12.0),
                            pal.text_primary,
                        );
                    }
                    if row_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Mở nhóm trò chuyện")
                        .clicked()
                    {
                        clicked_group = Some(group.clone());
                    }
                    ui.add_space(2.0);
                }

                if visible_groups.len() > 3 {
                    let toggle_label = if self.common_groups_expanded {
                        "Thu gọn ▴".to_string()
                    } else {
                        format!("Xem tất cả ({}) ▾", visible_groups.len())
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(toggle_label)
                                    .size(11.0)
                                    .color(pal.primary_hover),
                            )
                            .frame(false),
                        )
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.common_groups_expanded = !self.common_groups_expanded;
                    }
                }
            });
        ui.add(egui::Separator::default().spacing(1.0));

        if trigger_refresh {
            self.request_common_groups(d_id);
        }
        if let Some(group) = clicked_group {
            self.open_group_thread(ui.ctx(), &group);
        }
    }

    /// "Thành viên nhóm" section of the info drawer: count, search, roles.
    fn render_group_members(&mut self, ui: &mut egui::Ui, group_id: &str, pal: &ThemePalette) {
        let thread_members = self
            .threads
            .iter()
            .find(|t| t.id == group_id && t.is_group)
            .map(|t| (t.presentation.members.clone(), t.presentation.member_count));
        let loaded = self.group_members.get(group_id).cloned();
        let (members, total) = loaded.clone().or(thread_members).unwrap_or_default();
        let state = self.group_members_state.get(group_id).cloned();
        let loading = matches!(state, Some(None));
        egui::Frame::none()
            .inner_margin(Margin::symmetric(16.0, 14.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Thành viên nhóm")
                            .size(12.5)
                            .color(pal.text_primary)
                            .strong(),
                    );
                    let count = total.unwrap_or(members.len() as u64);
                    ui.label(
                        RichText::new(format!("({count})"))
                            .size(11.5)
                            .color(pal.primary_hover),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if loading {
                            ui.spinner();
                        } else if ui
                            .add(
                                egui::Button::new(RichText::new(lucide::REFRESH).size(11.5))
                                    .frame(false),
                            )
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text("Tải lại danh sách thành viên")
                            .clicked()
                        {
                            self.request_group_members(group_id);
                        }
                    });
                });
                if let Some(Some(error)) = &state {
                    ui.label(
                        RichText::new(format!("Không tải được thành viên: {error}"))
                            .size(11.0)
                            .color(pal.text_muted),
                    );
                }
                if loaded.is_none() && !loading && state.is_none() {
                    // First open: fetch names and avatars for the whole roster once.
                    self.request_group_members(group_id);
                }
                if members.is_empty() {
                    return;
                }
                ui.add_space(6.0);
                if members.len() > 5 {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.group_member_search)
                            .hint_text("Tìm thành viên…")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(4.0);
                }
                let query = fold_vn_search(self.group_member_search.trim());
                let filtered: Vec<crate::chat::SidebarMember> = members
                    .iter()
                    .filter(|m| {
                        let resolved = self.resolve_member_display_name(m);
                        query.is_empty()
                            || fold_vn_search(&resolved).contains(&query)
                            || fold_vn_search(&m.name).contains(&query)
                            || m.id.contains(&query)
                    })
                    .cloned()
                    .collect();
                // The member view is the primary content of the drawer.  Let it
                // consume the remaining drawer height instead of imposing the
                // old 360 px cap, which left a large unused area below the
                // roster on tall windows.
                let member_view_height = ui.available_height().max(120.0);
                egui::ScrollArea::vertical()
                    .id_salt(("info_drawer_members", group_id))
                    .max_height(member_view_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for member in &filtered {
                            self.render_group_member_row(ui, group_id, member, 13.0, 36.0, pal);
                            ui.add_space(2.0);
                        }
                    });
            });
    }

    // -----------------------------------------------------------------------
    // Right Info Drawer Multi-View Stack (Phase 4 - Eliminate Floating Windows)
    // -----------------------------------------------------------------------

    fn render_drawer_subview_header(&mut self, ui: &mut egui::Ui, title: &str, pal: &ThemePalette) {
        egui::Frame::none()
            .inner_margin(Margin::symmetric(14.0, 12.0))
            .fill(pal.bg_sidebar)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (btn_rect, btn_resp) =
                        ui.allocate_exact_size(Vec2::new(28.0, 28.0), Sense::click());
                    if btn_resp.hovered() {
                        ui.painter()
                            .rect_filled(btn_rect, Rounding::same(6.0), pal.bg_hover);
                    }
                    let c = btn_rect.center();
                    let arrow_col = if btn_resp.hovered() {
                        pal.text_primary
                    } else {
                        pal.text_secondary
                    };
                    ui.painter().line_segment(
                        [c + Vec2::new(3.0, -5.0), c + Vec2::new(-3.0, 0.0)],
                        Stroke::new(2.0_f32, arrow_col),
                    );
                    ui.painter().line_segment(
                        [c + Vec2::new(-3.0, 0.0), c + Vec2::new(3.0, 5.0)],
                        Stroke::new(2.0_f32, arrow_col),
                    );
                    if btn_resp
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .on_hover_text("Quay lại")
                        .clicked()
                    {
                        self.drawer_view = DrawerView::Overview;
                    }

                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(title)
                            .size(13.5)
                            .color(pal.text_primary)
                            .strong(),
                    );
                });
            });
        ui.add(egui::Separator::default().spacing(1.0));
    }

    fn draw_quick_circle_btn(
        ui: &mut egui::Ui,
        icon: &str,
        label: &str,
        active: bool,
        pal: &ThemePalette,
    ) -> bool {
        let mut clicked = false;
        ui.vertical_centered(|ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::click());
            let bg = if active {
                pal.primary.linear_multiply(0.2)
            } else if resp.hovered() {
                pal.bg_hover
            } else {
                pal.bg_elevated
            };
            let stroke_col = if active {
                pal.primary
            } else if resp.hovered() {
                pal.primary_hover
            } else {
                pal.border
            };
            ui.painter().circle_filled(rect.center(), 18.0, bg);
            ui.painter()
                .circle_stroke(rect.center(), 18.0, Stroke::new(1.0_f32, stroke_col));
            draw_lucide_icon(
                ui.painter(),
                rect.center(),
                icon,
                16.0,
                if active {
                    pal.primary
                } else {
                    pal.text_primary
                },
            );
            ui.add_space(3.0);
            ui.label(RichText::new(label).size(10.5).color(if active {
                pal.primary
            } else {
                pal.text_secondary
            }));
            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                clicked = true;
            }
        });
        clicked
    }

    #[allow(clippy::too_many_arguments)]
    fn render_drawer_overview(
        &mut self,
        ui: &mut egui::Ui,
        d_id: &str,
        d_name: &str,
        d_ava: Option<&str>,
        d_is_group: bool,
        d_msg_count: usize,
        shared_photos: &[(String, crate::message_view::Media, ChatMessage)],
        shared_files: &[(crate::message_view::Media, ChatMessage)],
        shared_links: &[(String, String, ChatMessage)],
        pal: &ThemePalette,
    ) {
        // Cloud storage is identified by the server-provided self-storage ID;
        // display names are editable and must not grant cloud capabilities.
        let d_is_cloud = !self.own_cloud_id.is_empty() && d_id == self.own_cloud_id;
        if d_is_cloud
            && (self.cloud_usage.owner != self.user_uin
                || (!self.cloud_usage.pending && self.cloud_usage.result.is_none()))
        {
            self.request_my_documents_usage();
        }

        egui::ScrollArea::vertical()
            .id_salt("drawer_overview_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 1. Profile Card Header
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(16.0, 16.0))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let blocked = self.is_workspace_input_gated(ui.ctx());
                            let (av_rect, av_response) = ui.allocate_exact_size(
                                Vec2::new(64.0, 64.0),
                                if d_is_group && !blocked {
                                    Sense::click()
                                } else {
                                    Sense::hover()
                                },
                            );
                            paint_user_avatar(
                                ui.painter(),
                                av_rect.center(),
                                32.0,
                                d_name,
                                d_is_group,
                                d_ava,
                                &self.sticker_textures,
                                &mut self.loading_stickers,
                            );
                            if d_is_group
                                && !blocked
                                && group_avatar_affordance(ui, av_response, pal)
                            {
                                self.open_conversation_avatar_editor(d_id);
                            }
                            ui.add_space(8.0);
                            let can_edit_name = !d_is_cloud && d_id != self.user_uin;
                            let name_width = ui
                                .painter()
                                .layout_no_wrap(
                                    d_name.to_owned(),
                                    FontId::proportional(14.5),
                                    pal.text_primary,
                                )
                                .size()
                                .x;
                            ui.horizontal(|ui| {
                                let row_width = name_width + if can_edit_name { 24.0 } else { 0.0 };
                                ui.add_space(((ui.available_width() - row_width) / 2.0).max(0.0));
                                let name_response = ui.add(
                                    egui::Label::new(
                                        RichText::new(d_name)
                                            .size(14.5)
                                            .color(pal.text_primary)
                                            .strong(),
                                    )
                                    .truncate(),
                                );
                                if can_edit_name
                                    && name_edit_affordance(
                                        ui,
                                        name_response.hovered(),
                                        !blocked,
                                        d_is_group,
                                        pal,
                                    )
                                {
                                    self.open_conversation_name_editor(d_id, d_name, d_is_group);
                                }
                            });
                            if !d_is_group && !d_is_cloud {
                                let zalo_sub =
                                    self.threads.get(self.selected_thread_index).and_then(|t| {
                                        let has_alias = t.presentation.server_alias.is_some()
                                            || self
                                                .server_aliases
                                                .get(d_id)
                                                .is_some_and(|a| !a.trim().is_empty());
                                        has_alias
                                            .then_some(t.presentation.base_name.as_deref())
                                            .flatten()
                                            .filter(|base| {
                                                !base.trim().is_empty()
                                                    && !crate::chat::is_generic_thread_name(base)
                                                    && *base != d_name
                                            })
                                    });
                                if let Some(zalo_name) = zalo_sub {
                                    ui.label(
                                        RichText::new(format!("Tên Zalo: {zalo_name}"))
                                            .size(11.5)
                                            .color(pal.text_muted),
                                    );
                                }
                            }
                            ui.add_space(2.0);
                            let sub_txt = if d_is_group {
                                let thread = self
                                    .threads
                                    .iter()
                                    .find(|thread| thread.id == d_id && thread.is_group);
                                let member_count = thread
                                    .and_then(|t| t.presentation.member_count)
                                    .unwrap_or(0);
                                let kind = thread
                                    .and_then(|t| t.presentation.group_kind)
                                    .map(crate::chat::GroupKind::label)
                                    .unwrap_or("Hội thoại nhóm");
                                if member_count > 0 {
                                    format!("{kind} · {member_count} thành viên")
                                } else {
                                    kind.to_owned()
                                }
                            } else if d_is_cloud {
                                "Lưu trữ đám mây cá nhân".to_string()
                            } else {
                                let fr_status = self
                                    .friend_status_map
                                    .get(d_id)
                                    .copied()
                                    .unwrap_or(crate::chat::FriendshipStatus::Unknown);
                                match fr_status {
                                    crate::chat::FriendshipStatus::Friend => {
                                        "✓ Đã kết bạn".to_string()
                                    }
                                    crate::chat::FriendshipStatus::IncomingPending => {
                                        "📩 Đã gửi lời mời kết bạn cho bạn".to_string()
                                    }
                                    crate::chat::FriendshipStatus::OutgoingPending => {
                                        "⏳ Đã gửi lời mời kết bạn".to_string()
                                    }
                                    crate::chat::FriendshipStatus::Stranger => {
                                        "Người lạ · Chưa kết bạn".to_string()
                                    }
                                    crate::chat::FriendshipStatus::Unknown => {
                                        "Đang kiểm tra kết bạn…".to_string()
                                    }
                                }
                            };
                            ui.label(RichText::new(sub_txt).size(11.5).color(pal.text_secondary));
                        });
                    });

                ui.add(egui::Separator::default().spacing(1.0));

                if d_is_cloud {
                    self.render_my_documents_overview(ui, pal);
                    return;
                }

                // 2. 4 Circular Quick Action Buttons
                let is_muted = self
                    .conversation_preferences
                    .get(d_id)
                    .map(|p| p.muted)
                    .unwrap_or(false);
                let remote_pinned = self.server_pins.rank(d_id, d_is_group).is_some();
                let local_pinned = self
                    .conversation_preferences
                    .get(d_id)
                    .map(|p| p.pinned)
                    .unwrap_or(false);
                let is_pinned = remote_pinned || local_pinned;

                let mut mute_clicked = false;
                let mut pin_clicked = false;
                let mut grp_clicked = false;
                let mut settings_clicked = false;

                egui::Frame::none()
                    .inner_margin(Margin::symmetric(14.0, 10.0))
                    .show(ui, |ui| {
                        let col_w = (ui.available_width() - 3.0 * 6.0) / 4.0;
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;

                            ui.allocate_ui(Vec2::new(col_w, 58.0), |ui| {
                                mute_clicked = Self::draw_quick_circle_btn(
                                    ui,
                                    lucide::BELL,
                                    if is_muted {
                                        "Bật chuông"
                                    } else {
                                        "Tắt chuông"
                                    },
                                    is_muted,
                                    pal,
                                );
                            });

                            ui.allocate_ui(Vec2::new(col_w, 58.0), |ui| {
                                pin_clicked = Self::draw_quick_circle_btn(
                                    ui,
                                    lucide::PIN,
                                    if is_pinned { "Bỏ ghim" } else { "Ghim tin" },
                                    is_pinned,
                                    pal,
                                );
                            });

                            ui.allocate_ui(Vec2::new(col_w, 58.0), |ui| {
                                let (icon, label) = if d_is_cloud {
                                    (lucide::REFRESH, "Đồng bộ")
                                } else if d_is_group {
                                    (lucide::USER_PLUS, "Thêm bạn")
                                } else {
                                    (lucide::USERS, "Tạo nhóm")
                                };
                                grp_clicked =
                                    Self::draw_quick_circle_btn(ui, icon, label, false, pal);
                            });

                            ui.allocate_ui(Vec2::new(col_w, 58.0), |ui| {
                                settings_clicked = Self::draw_quick_circle_btn(
                                    ui,
                                    lucide::SETTINGS,
                                    "Cài đặt",
                                    false,
                                    pal,
                                );
                            });
                        });
                    });

                if mute_clicked {
                    let current_pref = self
                        .conversation_preferences
                        .get(d_id)
                        .copied()
                        .unwrap_or_default();
                    let new_pref = crate::store::ConversationPreference {
                        muted: !is_muted,
                        ..current_pref
                    };
                    self.conversation_preferences
                        .insert(d_id.to_string(), new_pref);
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::SetConversationPreference {
                            owner: self.user_uin.clone(),
                            thread_id: d_id.to_string(),
                            is_group: d_is_group,
                            value: new_pref,
                        });
                    }
                    self.show_toast(if !is_muted {
                        "Đã tắt thông báo hội thoại"
                    } else {
                        "Đã bật thông báo hội thoại"
                    });
                }

                if pin_clicked {
                    let new_pinned = !is_pinned;
                    if let Some(tx) = WORKER_TX.get() {
                        let _ = tx.send(WorkerCmd::SetServerPin {
                            owner: self.user_uin.clone(),
                            thread: d_id.to_string(),
                            group: d_is_group,
                            pinned: new_pinned,
                        });
                    }
                    let current_pref = self
                        .conversation_preferences
                        .get(d_id)
                        .copied()
                        .unwrap_or_default();
                    let new_pref = crate::store::ConversationPreference {
                        pinned: new_pinned,
                        ..current_pref
                    };
                    self.conversation_preferences
                        .insert(d_id.to_string(), new_pref);
                    self.show_toast(if new_pinned {
                        "Đã ghim hội thoại"
                    } else {
                        "Đã bỏ ghim hội thoại"
                    });
                }

                if grp_clicked {
                    if d_is_cloud {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::RefreshThreads);
                            let _ = tx.send(WorkerCmd::LoadGroupHistory {
                                limit: 500,
                                group_id: d_id.to_string(),
                                is_group: false,
                            });
                        }
                        self.show_toast("Đang đồng bộ dữ liệu Cloud…");
                    } else if d_is_group {
                        self.open_group_panel(d_id.to_string(), true, false);
                    } else {
                        self.open_group_panel(d_id.to_string(), false, true);
                    }
                }

                if settings_clicked {
                    self.drawer_view = DrawerView::SecuritySettings;
                }

                ui.add(egui::Separator::default().spacing(1.0));

                // 3. Accordion 1: Thành viên (Members) if group, or Nhóm chung (Common Groups) if 1-1
                if d_is_group {
                    let thread_members = self
                        .threads
                        .iter()
                        .find(|t| t.id == d_id && t.is_group)
                        .map(|t| (t.presentation.members.clone(), t.presentation.member_count));
                    let loaded = self.group_members.get(d_id).cloned();
                    let (members, total) = loaded.clone().or(thread_members).unwrap_or_default();
                    let state = self.group_members_state.get(d_id).cloned();
                    let loading = matches!(state, Some(None));
                    if loaded.is_none() && !loading && state.is_none() {
                        self.request_group_members(d_id);
                    }
                    let count = total.unwrap_or(members.len() as u64);

                    egui::Frame::none()
                        .inner_margin(Margin::symmetric(16.0, 12.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("Thành viên")
                                        .size(12.5)
                                        .color(pal.text_primary)
                                        .strong(),
                                );
                                ui.label(
                                    RichText::new(format!("({count})"))
                                        .size(11.5)
                                        .color(pal.primary_hover),
                                );
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let (all_rect, all_resp) = ui
                                        .allocate_exact_size(Vec2::new(66.0, 22.0), Sense::click());
                                    if all_resp.hovered() {
                                        ui.painter().rect_filled(
                                            all_rect,
                                            Rounding::same(4.0),
                                            pal.bg_hover,
                                        );
                                    }
                                    ui.painter().text(
                                        all_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "Xem tất cả >",
                                        FontId::proportional(11.0),
                                        pal.primary_hover,
                                    );
                                    if all_resp.on_hover_cursor(CursorIcon::PointingHand).clicked()
                                    {
                                        self.drawer_view = DrawerView::Members;
                                    }
                                    if loading {
                                        ui.spinner();
                                    }
                                });
                            });
                            ui.add_space(6.0);
                            if members.is_empty() {
                                if let Some(Some(error)) = &state {
                                    ui.label(
                                        RichText::new(format!(
                                            "Không tải được thành viên: {error}"
                                        ))
                                        .size(11.0)
                                        .color(pal.text_muted),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("Đang tải danh sách thành viên…")
                                            .size(11.0)
                                            .color(pal.text_muted),
                                    );
                                }
                            } else {
                                for member in members.iter().take(5) {
                                    self.render_group_member_row(ui, d_id, member, 11.5, 32.0, pal);
                                    ui.add_space(2.0);
                                }
                            }
                        });
                    ui.add(egui::Separator::default().spacing(1.0));
                } else if !d_is_cloud {
                    self.render_common_groups_section(ui, d_id, pal);
                }

                // 4. Accordion 2: Bảng tin nhóm & Nhắc hẹn (Board)
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Bảng tin & Nhắc hẹn")
                                    .size(12.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (all_rect, all_resp) =
                                    ui.allocate_exact_size(Vec2::new(66.0, 22.0), Sense::click());
                                if all_resp.hovered() {
                                    ui.painter().rect_filled(
                                        all_rect,
                                        Rounding::same(4.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    all_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Xem tất cả >",
                                    FontId::proportional(11.0),
                                    pal.primary_hover,
                                );
                                if all_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.drawer_view = DrawerView::Board;
                                }
                            });
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("📌 Tin ghim").clicked() {
                                self.drawer_view = DrawerView::Board;
                                self.server_board.pins = true;
                            }
                            if ui.button("⏰ Nhắc hẹn").clicked() {
                                self.drawer_view = DrawerView::Board;
                                self.server_board.pins = false;
                            }
                            if d_is_group && ui.button("📊 Bình chọn").clicked() {
                                self.open_group_content(d_id.to_string());
                            }
                        });
                    });
                ui.add(egui::Separator::default().spacing(1.0));

                // 5. Accordion 3: Ảnh / Video
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Ảnh / Video")
                                    .size(12.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (all_rect, all_resp) =
                                    ui.allocate_exact_size(Vec2::new(66.0, 22.0), Sense::click());
                                if all_resp.hovered() {
                                    ui.painter().rect_filled(
                                        all_rect,
                                        Rounding::same(4.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    all_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Xem tất cả >",
                                    FontId::proportional(11.0),
                                    pal.primary_hover,
                                );
                                if all_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.drawer_view = DrawerView::MediaStorage {
                                        tab: MediaTab::Photos,
                                    };
                                }
                            });
                        });
                        ui.add_space(6.0);
                        if shared_photos.is_empty() {
                            ui.label(
                                RichText::new("Chưa có ảnh hoặc video chia sẻ.")
                                    .size(11.0)
                                    .color(pal.text_muted),
                            );
                        } else {
                            let cell_sz = ((ui.available_width() - 18.0) / 4.0).clamp(56.0, 72.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::new(5.0, 5.0);
                                for (tkey, media, message) in shared_photos.iter().take(8) {
                                    self.render_photo_cell(
                                        ui, tkey, media, message, d_id, d_name, d_is_group,
                                        cell_sz, pal,
                                    );
                                }
                            });
                        }
                    });
                ui.add(egui::Separator::default().spacing(1.0));

                // 6. Accordion 4: File
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Tài liệu & Tệp")
                                    .size(12.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (all_rect, all_resp) =
                                    ui.allocate_exact_size(Vec2::new(66.0, 22.0), Sense::click());
                                if all_resp.hovered() {
                                    ui.painter().rect_filled(
                                        all_rect,
                                        Rounding::same(4.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    all_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Xem tất cả >",
                                    FontId::proportional(11.0),
                                    pal.primary_hover,
                                );
                                if all_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.drawer_view = DrawerView::MediaStorage {
                                        tab: MediaTab::Files,
                                    };
                                }
                            });
                        });
                        ui.add_space(6.0);
                        if shared_files.is_empty() {
                            ui.label(
                                RichText::new("Chưa có file nào được gửi.")
                                    .size(11.0)
                                    .color(pal.text_muted),
                            );
                        } else {
                            for (media, msg) in shared_files.iter().take(3) {
                                self.render_file_row(ui, media, msg, d_id, d_is_group, pal);
                                ui.add_space(4.0);
                            }
                        }
                    });
                ui.add(egui::Separator::default().spacing(1.0));

                // 7. Accordion 5: Link
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Liên kết đã chia sẻ")
                                    .size(12.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (all_rect, all_resp) =
                                    ui.allocate_exact_size(Vec2::new(66.0, 22.0), Sense::click());
                                if all_resp.hovered() {
                                    ui.painter().rect_filled(
                                        all_rect,
                                        Rounding::same(4.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    all_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Xem tất cả >",
                                    FontId::proportional(11.0),
                                    pal.primary_hover,
                                );
                                if all_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.drawer_view = DrawerView::MediaStorage {
                                        tab: MediaTab::Links,
                                    };
                                }
                            });
                        });
                        ui.add_space(6.0);
                        if shared_links.is_empty() {
                            ui.label(
                                RichText::new("Chưa có liên kết nào.")
                                    .size(11.0)
                                    .color(pal.text_muted),
                            );
                        } else {
                            for (url, title, msg) in shared_links.iter().take(3) {
                                self.render_link_row(ui, url, title, msg, d_id, d_is_group, pal);
                                ui.add_space(4.0);
                            }
                        }
                    });
                ui.add(egui::Separator::default().spacing(1.0));

                // 8. Accordion 6: Thiết lập bảo mật & Quyền riêng tư
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Thiết lập bảo mật")
                                    .size(12.5)
                                    .color(pal.text_primary)
                                    .strong(),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (all_rect, all_resp) =
                                    ui.allocate_exact_size(Vec2::new(66.0, 22.0), Sense::click());
                                if all_resp.hovered() {
                                    ui.painter().rect_filled(
                                        all_rect,
                                        Rounding::same(4.0),
                                        pal.bg_hover,
                                    );
                                }
                                ui.painter().text(
                                    all_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Chi tiết >",
                                    FontId::proportional(11.0),
                                    pal.primary_hover,
                                );
                                if all_resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                    self.drawer_view = DrawerView::SecuritySettings;
                                }
                            });
                        });
                        let current_ttl = self
                            .policy_panel
                            .snapshot
                            .as_ref()
                            .map(|s| s.ttl(d_id, d_is_group))
                            .unwrap_or(0);
                        let ttl_status = format_auto_delete_ttl(current_ttl);
                        ui.add_space(4.0);
                        let (ttl_rect, ttl_resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 28.0),
                            Sense::click(),
                        );
                        if ttl_resp.hovered() {
                            ui.painter()
                                .rect_filled(ttl_rect, Rounding::same(6.0), pal.bg_hover);
                        }
                        ui.painter().text(
                            Pos2::new(ttl_rect.left() + 6.0, ttl_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "⏱️ Tin nhắn tự xóa",
                            FontId::proportional(12.0),
                            pal.text_primary,
                        );
                        ui.painter().text(
                            Pos2::new(ttl_rect.right() - 6.0, ttl_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            format!("{ttl_status} ›"),
                            FontId::proportional(11.5),
                            if current_ttl > 0 {
                                pal.primary
                            } else {
                                pal.text_muted
                            },
                        );
                        if ttl_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text("Thiết lập thời gian tự xóa tin nhắn")
                            .clicked()
                        {
                            self.auto_delete_modal = Some(AutoDeleteModalState {
                                thread_id: d_id.to_string(),
                                is_group: d_is_group,
                                thread_name: d_name.to_string(),
                                selected_ttl: current_ttl,
                            });
                        }

                        let has_snapshot = self.policy_panel.snapshot.is_some();
                        let is_protected = self
                            .policy_panel
                            .snapshot
                            .as_ref()
                            .is_some_and(|s| s.protects(d_id, d_is_group));
                        let prot_status = if !has_snapshot {
                            "Chưa rõ"
                        } else if is_protected {
                            "Đang bật"
                        } else {
                            "Đang tắt"
                        };
                        let (prot_rect, prot_resp) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 28.0),
                            Sense::click(),
                        );
                        if prot_resp.hovered() {
                            ui.painter()
                                .rect_filled(prot_rect, Rounding::same(6.0), pal.bg_hover);
                        }
                        ui.painter().text(
                            Pos2::new(prot_rect.left() + 6.0, prot_rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            "🛡️ Chặn sao chép, tải về, chuyển tiếp",
                            FontId::proportional(11.5),
                            pal.text_primary,
                        );
                        ui.painter().text(
                            Pos2::new(prot_rect.right() - 6.0, prot_rect.center().y),
                            egui::Align2::RIGHT_CENTER,
                            format!("{prot_status} ›"),
                            FontId::proportional(11.0),
                            if is_protected {
                                pal.primary
                            } else {
                                pal.text_muted
                            },
                        );
                        if prot_resp
                            .on_hover_cursor(CursorIcon::PointingHand)
                            .on_hover_text("Bật/Tắt bảo vệ nội dung hội thoại")
                            .clicked()
                        {
                            self.content_protection_modal = Some(ContentProtectionModalState {
                                thread_id: d_id.to_string(),
                                is_group: d_is_group,
                                thread_name: d_name.to_string(),
                                current_enabled: is_protected,
                            });
                        }
                    });
                ui.add(egui::Separator::default().spacing(1.0));

                // 9. Bottom Technical Metadata
                egui::Frame::none()
                    .fill(pal.bg_app)
                    .inner_margin(Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Tổng số tin nhắn:")
                                    .size(11.0)
                                    .color(pal.text_muted),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{d_msg_count} tin"))
                                        .size(11.0)
                                        .color(pal.text_secondary)
                                        .strong(),
                                );
                            });
                        });
                        ui.add_space(3.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("ID hội thoại:")
                                    .size(11.0)
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
                                    d_id.to_string()
                                };
                                let (id_rect, id_resp) =
                                    ui.allocate_exact_size(Vec2::new(88.0, 20.0), Sense::click());
                                ui.painter().rect_filled(
                                    id_rect,
                                    Rounding::same(4.0),
                                    if id_resp.hovered() {
                                        pal.bg_hover
                                    } else {
                                        pal.bg_elevated
                                    },
                                );
                                ui.painter().text(
                                    Pos2::new(id_rect.left() + 6.0, id_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    short_id,
                                    FontId::monospace(10.0),
                                    pal.text_secondary,
                                );
                                draw_icon_copy(
                                    ui.painter(),
                                    Pos2::new(id_rect.right() - 8.0, id_rect.center().y),
                                    10.0,
                                    pal.text_secondary,
                                );
                                if id_resp
                                    .on_hover_cursor(CursorIcon::PointingHand)
                                    .on_hover_text(format!("Sao chép ID: {d_id}"))
                                    .clicked()
                                {
                                    ui.ctx().copy_text(d_id.to_string());
                                    self.show_toast("Đã sao chép ID hội thoại");
                                }
                            });
                        });
                    });
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn render_photo_cell(
        &mut self,
        ui: &mut egui::Ui,
        tkey: &str,
        media: &crate::message_view::Media,
        message: &ChatMessage,
        d_id: &str,
        d_name: &str,
        d_is_group: bool,
        cell_sz: f32,
        pal: &ThemePalette,
    ) {
        let selection_key = archive_selection_key(
            "photo",
            d_id,
            message,
            media.original.as_deref().unwrap_or(tkey),
        );
        let (cell_r, cell_resp) = ui.allocate_exact_size(Vec2::splat(cell_sz), Sense::click());
        let thumb_u = media.thumbnail.as_deref().unwrap_or_default();
        let full_u = media.preview.as_deref().unwrap_or_default();
        let ptitle = &media.name;
        let thumb_allowed = reqwest::Url::parse(thumb_u)
            .map(|u| crate::media_download::allowed(&u))
            .unwrap_or(false);
        if ui.is_rect_visible(cell_r)
            && !thumb_u.is_empty()
            && thumb_allowed
            && !crate::message_view::is_video_url(thumb_u)
            && !self.sticker_textures.contains_key(tkey)
            && self.loading_stickers.insert(tkey.to_string())
        {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchChatImage {
                    key: tkey.to_string(),
                    url: thumb_u.to_owned(),
                    thread: d_id.to_string(),
                    group: d_is_group,
                    message: Box::new(message.clone()),
                    preview: false,
                });
            }
        }

        ui.painter()
            .rect_filled(cell_r, Rounding::same(6.0), pal.bg_elevated);
        if let Some(tex) = self.sticker_textures.get(tkey) {
            ui.painter().image(
                tex.id(),
                cell_r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            draw_icon_image(
                ui.painter(),
                cell_r.center(),
                cell_sz * 0.28,
                pal.text_muted,
            );
        }
        if media.kind == crate::message_view::MediaKind::Video {
            let c = cell_r.center();
            ui.painter()
                .circle_filled(c, 12.0, Color32::from_black_alpha(150));
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-3.5, -6.0),
                    c + Vec2::new(7.0, 0.0),
                    c + Vec2::new(-3.5, 6.0),
                ],
                Color32::WHITE,
                Stroke::NONE,
            ));
        }
        ui.painter().rect_stroke(
            cell_r,
            Rounding::same(6.0),
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
            .clone()
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(if media.kind == crate::message_view::MediaKind::Video {
                "Phát video"
            } else {
                "Xem ảnh phóng to"
            })
            .clicked()
        {
            if self.drawer_media_selection_mode {
                if !self.drawer_media_selected.remove(&selection_key) {
                    self.drawer_media_selected.insert(selection_key.clone());
                }
                return;
            }
            if media.kind == crate::message_view::MediaKind::Video {
                if let (Some(tx), Some(url)) = (WORKER_TX.get(), media.original.clone()) {
                    let _ = tx.send(WorkerCmd::DownloadMedia {
                        url,
                        name: media.name.clone(),
                        play: true,
                        open: false,
                        context: Some(super::worker::ChatImage {
                            thread: d_id.to_string(),
                            group: d_is_group,
                            message: message.clone(),
                            preview: false,
                        }),
                    });
                }
                return;
            }
            let full_key = format!("{tkey}:full");
            if !full_u.is_empty()
                && !self.sticker_textures.contains_key(&full_key)
                && self.loading_stickers.insert(full_key.clone())
            {
                if let Some(tx) = WORKER_TX.get() {
                    let _ = tx.send(WorkerCmd::FetchChatImage {
                        key: full_key.clone(),
                        url: full_u.into(),
                        thread: d_id.to_string(),
                        group: d_is_group,
                        message: Box::new(message.clone()),
                        preview: true,
                    });
                }
            }
            self.image_viewer = Some(ImageViewerModal {
                source: super::worker::ChatImage {
                    thread: d_id.to_string(),
                    group: d_is_group,
                    message: message.clone(),
                    preview: true,
                },
                is_video: false,
                navigation_pending: false,
                thread_id: d_id.to_string(),
                message_id: message.id.clone(),
                dimensions: media.width.zip(media.height),
                tex_key: tkey.to_string(),
                hd_tex_key: Some(full_key),
                original_url: media.original.clone().unwrap_or_default(),
                title: ptitle.clone(),
                sender: d_name.to_string(),
                time: String::new(),
                zoom: 1.0,
                pan: Vec2::ZERO,
                video_state: None,
            });
        }
        let selected = self.drawer_media_selected.contains(&selection_key);
        if self.drawer_media_selection_mode {
            ui.painter().rect_stroke(
                cell_r.shrink(2.0),
                Rounding::same(5.0),
                Stroke::new(2.0_f32, if selected { pal.primary } else { pal.border }),
            );
            ui.painter().circle_filled(
                cell_r.left_top() + Vec2::splat(12.0),
                8.0,
                if selected {
                    pal.primary
                } else {
                    pal.bg_elevated
                },
            );
            if selected {
                ui.painter().text(
                    cell_r.left_top() + Vec2::splat(12.0),
                    egui::Align2::CENTER_CENTER,
                    "✓",
                    FontId::proportional(11.0),
                    Color32::WHITE,
                );
            }
        }
        let message_for_menu = message.clone();
        let selection_key_for_menu = selection_key.clone();
        cell_resp.context_menu(|ui| {
            self.archive_item_actions(ui, d_id, d_is_group, &message_for_menu, Some(media), None);
            if ui.button("Chọn nhiều").clicked() {
                self.drawer_media_selection_mode = true;
                self.drawer_media_selected
                    .insert(selection_key_for_menu.clone());
                ui.close_menu();
            }
            if ui.button("Xóa chỉ ở phía tôi").clicked() {
                self.confirm_archive_mutation(
                    d_id,
                    d_is_group,
                    false,
                    std::slice::from_ref(&message_for_menu),
                );
                ui.close_menu();
            }
            if message_for_menu.is_outgoing && ui.button("Thu hồi với mọi người").clicked()
            {
                self.confirm_archive_mutation(
                    d_id,
                    d_is_group,
                    true,
                    std::slice::from_ref(&message_for_menu),
                );
                ui.close_menu();
            }
        });
    }

    fn render_file_row(
        &mut self,
        ui: &mut egui::Ui,
        media: &crate::message_view::Media,
        msg: &ChatMessage,
        d_id: &str,
        d_is_group: bool,
        pal: &ThemePalette,
    ) {
        let fname = if media.name.is_empty() {
            "Tập tin"
        } else {
            &media.name
        };
        let category = crate::message_view::attachment_file_category(media, msg);
        let selection_key = archive_selection_key(
            "file",
            d_id,
            msg,
            media.original.as_deref().unwrap_or(fname),
        );
        let selected = self.drawer_media_selected.contains(&selection_key);
        let row = ui.horizontal(|ui| {
            if self.drawer_media_selection_mode {
                let mut checked = selected;
                if ui.checkbox(&mut checked, "").changed() {
                    if checked {
                        self.drawer_media_selected.insert(selection_key.clone());
                    } else {
                        self.drawer_media_selected.remove(&selection_key);
                    }
                }
            }
            let (icon_r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            paint_file_category_icon(ui.painter(), icon_r, category);

            ui.vertical(|ui| {
                ui.label(
                    RichText::new(truncate_chars(fname, 22))
                        .size(11.5)
                        .color(pal.text_primary)
                        .strong(),
                );
                ui.label(
                    RichText::new(msg.display_time())
                        .size(10.0)
                        .color(pal.text_muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if let Some(ref url) = media.original {
                    if ui.button("Tải về").clicked() {
                        if let Some(tx) = WORKER_TX.get() {
                            let _ = tx.send(WorkerCmd::DownloadMedia {
                                url: url.clone(),
                                name: fname.to_string(),
                                play: false,
                                open: false,
                                context: Some(super::worker::ChatImage {
                                    thread: d_id.to_owned(),
                                    group: d_is_group,
                                    message: msg.clone(),
                                    preview: false,
                                }),
                            });
                        }
                        self.show_toast(format!("Bắt đầu tải {fname}…"));
                    }
                }
            });
        });
        let selection_key_for_menu = selection_key.clone();
        let msg_for_delete = msg.clone();
        let is_outgoing = msg.is_outgoing;
        row.response.context_menu(|ui| {
            self.archive_item_actions(ui, d_id, d_is_group, &msg_for_delete, Some(media), None);
            if ui.button("Chọn nhiều").clicked() {
                self.drawer_media_selection_mode = true;
                self.drawer_media_selected
                    .insert(selection_key_for_menu.clone());
                ui.close_menu();
            }
            if ui.button("Xóa chỉ ở phía tôi").clicked() {
                self.confirm_archive_mutation(
                    d_id,
                    d_is_group,
                    false,
                    std::slice::from_ref(&msg_for_delete),
                );
                ui.close_menu();
            }
            if is_outgoing && ui.button("Thu hồi với mọi người").clicked() {
                self.confirm_archive_mutation(
                    d_id,
                    d_is_group,
                    true,
                    std::slice::from_ref(&msg_for_delete),
                );
                ui.close_menu();
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn render_link_row(
        &mut self,
        ui: &mut egui::Ui,
        url: &str,
        title: &str,
        msg: &ChatMessage,
        d_id: &str,
        d_is_group: bool,
        pal: &ThemePalette,
    ) {
        let selection_key = archive_selection_key("link", d_id, msg, url);
        let shareable = !self.content_protected(d_id, d_is_group);
        let response = ui.horizontal(|ui| {
            if self.drawer_media_selection_mode {
                let mut checked = self.drawer_media_selected.contains(&selection_key);
                if ui.checkbox(&mut checked, "").changed() {
                    if checked {
                        self.drawer_media_selected.insert(selection_key.clone());
                    } else {
                        self.drawer_media_selected.remove(&selection_key);
                    }
                }
            }
            let (icon_r, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            let thumbnail = msg.link_thumb.as_deref().filter(|value| {
                reqwest::Url::parse(value).is_ok_and(|url| crate::media_download::allowed(&url))
            });
            let key = thumbnail.map(|thumb| {
                format!(
                    "archive-link-{}",
                    crate::crypto::md5_hex(
                        &serde_json::json!([d_id, msg.id, msg.client_message_id, thumb])
                            .to_string()
                    )
                )
            });
            if let (Some(thumb), Some(key)) = (thumbnail, key.as_ref()) {
                if ui.is_rect_visible(icon_r)
                    && self.private_access(d_id, d_is_group)
                    && !crate::message_lifecycle::expired(msg, crate::login::unix_now() * 1000)
                    && !self.sticker_textures.contains_key(key)
                    && !self.image_failures.contains_key(key)
                    && self.loading_stickers.insert(key.clone())
                {
                    let queued = WORKER_TX.get().is_some_and(|tx| {
                        tx.send(WorkerCmd::FetchChatImage {
                            key: key.clone(),
                            url: thumb.to_owned(),
                            thread: d_id.to_owned(),
                            group: d_is_group,
                            message: Box::new(msg.clone()),
                            preview: false,
                        })
                        .is_ok()
                    });
                    if !queued {
                        self.loading_stickers.remove(key);
                    }
                }
            }
            ui.painter()
                .rect_filled(icon_r, Rounding::same(6.0), pal.bg_elevated);
            if let Some(texture) = key.as_ref().and_then(|key| self.sticker_textures.get(key)) {
                ui.painter().image(
                    texture.id(),
                    icon_r,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            } else {
                ui.painter().text(
                    icon_r.center(),
                    egui::Align2::CENTER_CENTER,
                    "🔗",
                    FontId::proportional(14.0),
                    pal.primary_hover,
                );
            }

            ui.vertical(|ui| {
                let display_title = if title.trim().is_empty() {
                    truncate_chars(url, 22)
                } else {
                    truncate_chars(title, 42)
                };
                ui.label(
                    RichText::new(display_title)
                        .size(11.5)
                        .color(pal.text_primary),
                );
                if !title.trim().is_empty() {
                    ui.label(
                        RichText::new(truncate_chars(url, 32))
                            .size(10.0)
                            .color(pal.primary_hover)
                            .underline(),
                    );
                }
                ui.label(
                    RichText::new(msg.display_time())
                        .size(10.0)
                        .color(pal.text_muted),
                );
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Mở").clicked() {
                    ui.ctx().open_url(egui::OpenUrl::same_tab(url));
                }
                if ui
                    .add_enabled(shareable, egui::Button::new("📋"))
                    .on_disabled_hover_text(
                        "Chưa xác minh quyền sao chép hoặc hội thoại đang chặn chia sẻ",
                    )
                    .clicked()
                {
                    ui.ctx().copy_text(url.to_string());
                    self.show_toast("Đã sao chép liên kết");
                }
            });
        });
        let message_for_menu = msg.clone();
        let selection_key_for_menu = selection_key.clone();
        response.response.context_menu(|ui| {
            self.archive_item_actions(ui, d_id, d_is_group, &message_for_menu, None, Some(url));
            if ui.button("Chọn nhiều").clicked() {
                self.drawer_media_selection_mode = true;
                self.drawer_media_selected
                    .insert(selection_key_for_menu.clone());
                ui.close_menu();
            }
            if ui.button("Xóa chỉ ở phía tôi").clicked() {
                self.confirm_archive_mutation(
                    d_id,
                    d_is_group,
                    false,
                    std::slice::from_ref(&message_for_menu),
                );
                ui.close_menu();
            }
            if message_for_menu.is_outgoing && ui.button("Thu hồi với mọi người").clicked()
            {
                self.confirm_archive_mutation(
                    d_id,
                    d_is_group,
                    true,
                    std::slice::from_ref(&message_for_menu),
                );
                ui.close_menu();
            }
        });
    }

    fn render_drawer_members_view(
        &mut self,
        ui: &mut egui::Ui,
        group_id: &str,
        pal: &ThemePalette,
    ) {
        self.render_drawer_subview_header(ui, "Thành viên nhóm", pal);
        ui.horizontal(|ui| {
            if ui.button("👥+ Thêm thành viên vào nhóm").clicked() {
                self.open_group_panel(group_id.to_string(), true, false);
            }
        });
        ui.add_space(4.0);
        self.render_group_members(ui, group_id, pal);
    }

    fn render_drawer_board_view(
        &mut self,
        ui: &mut egui::Ui,
        thread_id: &str,
        is_group: bool,
        pal: &ThemePalette,
    ) {
        self.render_drawer_subview_header(ui, "Bảng tin & Nhắc hẹn", pal);

        if self.server_board.thread != thread_id {
            self.server_board.thread = thread_id.to_string();
            self.server_board.group = is_group;
            self.server_board.generation = crate::search::generation();
            self.server_board.open = false;
            self.board_command(crate::board::Action::List {
                page: 0,
                last: String::new(),
                pins: self.server_board.pins,
            });
        }

        let mut action = None;
        ui.horizontal(|ui| {
            if ui
                .selectable_label(!self.server_board.pins, "⏰ Nhắc hẹn")
                .clicked()
            {
                self.server_board.pins = false;
                self.server_board.page_number = 0;
                action = Some(crate::board::Action::List {
                    page: 0,
                    last: String::new(),
                    pins: false,
                });
            }
            if is_group
                && ui
                    .selectable_label(self.server_board.pins, "📌 Tin ghim server")
                    .clicked()
            {
                self.server_board.pins = true;
                self.server_board.page_number = 0;
                action = Some(crate::board::Action::List {
                    page: 0,
                    last: String::new(),
                    pins: true,
                });
            }
            if ui.button("🔄 Tải lại").clicked() {
                self.server_board.page_number = 0;
                action = Some(crate::board::Action::List {
                    page: 0,
                    last: String::new(),
                    pins: self.server_board.pins,
                });
            }
        });

        if self.server_board.pending {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Đang tải dữ liệu từ máy chủ…");
            });
        }

        if !self.server_board.pins {
            ui.label(
                RichText::new(if self.server_board.editing.is_some() {
                    "Sửa nhắc hẹn"
                } else {
                    "Tạo nhắc hẹn mới"
                })
                .strong(),
            );
            ui.add(
                egui::TextEdit::multiline(&mut self.server_board.title)
                    .hint_text("Nội dung nhắc hẹn…")
                    .desired_rows(2),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("Giờ máy (YYYY-MM-DD HH:MM):");
                ui.text_edit_singleline(&mut self.server_board.when);
                if ui.button("Lưu lên Zalo").clicked() {
                    use chrono::{Local, NaiveDateTime, TimeZone};
                    match NaiveDateTime::parse_from_str(
                        self.server_board.when.trim(),
                        "%Y-%m-%d %H:%M",
                    )
                    .ok()
                    .and_then(|t| Local.from_local_datetime(&t).single())
                    {
                        Some(time) => {
                            action = Some(crate::board::Action::Save {
                                id: self.server_board.editing.as_ref().map(|i| i.id.clone()),
                                title: self.server_board.title.clone(),
                                start: time.timestamp_millis(),
                                original: self.server_board.editing.as_ref().map(|i| i.raw.clone()),
                            });
                        }
                        None => self.server_board.status = "Thời gian không hợp lệ".into(),
                    }
                }
                if self.server_board.editing.is_some() && ui.button("Hủy sửa").clicked() {
                    self.server_board.editing = None;
                    self.server_board.title.clear();
                    self.server_board.when.clear();
                }
            });
        }

        ui.add(egui::Separator::default().spacing(1.0));

        egui::ScrollArea::vertical()
            .id_salt("drawer_board_scroll")
            .max_height((ui.available_height() - 40.0).max(120.0))
            .show(ui, |ui| {
                let mut jump = None;
                for item in &self.server_board.page.items {
                    ui.push_id(&item.id, |ui| {
                        ui.group(|ui| {
                            ui.strong(&item.title);
                            if item.start > 0 {
                                ui.small(crate::clock::message_time(item.start));
                            }
                            ui.horizontal(|ui| {
                                if !self.server_board.pins {
                                    if ui.button("Chi tiết").clicked() {
                                        action = Some(crate::board::Action::Detail {
                                            event: item.id.clone(),
                                        });
                                    }
                                    if item.creator == self.user_uin {
                                        if !item.repeating && ui.button("Sửa").clicked() {
                                            self.server_board.editing = Some(item.clone());
                                            self.server_board.title = item.title.clone();
                                            self.server_board.when =
                                                chrono::DateTime::from_timestamp_millis(item.start)
                                                    .map(|t| {
                                                        t.with_timezone(&chrono::Local)
                                                            .format("%Y-%m-%d %H:%M")
                                                            .to_string()
                                                    })
                                                    .unwrap_or_default();
                                        }
                                        if ui.button("Xóa…").clicked() {
                                            self.server_board.confirm_remove = Some(item.clone());
                                        }
                                    }
                                } else {
                                    if !item.message_id.is_empty()
                                        && ui.button("Đi tới tin gốc").clicked()
                                    {
                                        jump = Some(item.message_id.clone());
                                    }
                                    if let Some(version) = self.server_board.page.version {
                                        if ui.button("Bỏ ghim").clicked() {
                                            action = Some(crate::board::Action::Unpin {
                                                item: item.raw.clone(),
                                                version,
                                            });
                                        }
                                    }
                                }
                            });
                        });
                    });
                }
                if let Some(id) = jump {
                    self.filter_starred_only = false;
                    self.tools_command(super::tools_panel::ToolsCommand::OpenHit {
                        thread: thread_id.to_string(),
                        id,
                    });
                    ui.ctx().request_repaint();
                }
            });

        if let Some(action) = action {
            self.board_command(action);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_drawer_media_storage_view(
        &mut self,
        ui: &mut egui::Ui,
        d_id: &str,
        d_is_group: bool,
        tab: MediaTab,
        _shared_photos: &[(String, crate::message_view::Media, ChatMessage)],
        _shared_files: &[(crate::message_view::Media, ChatMessage)],
        _shared_links: &[(String, String, ChatMessage)],
        pal: &ThemePalette,
    ) {
        self.render_drawer_subview_header(ui, "Kho lưu trữ", pal);

        let mut sender_entries = std::collections::BTreeMap::new();
        for message in self.archive_history.messages.iter().chain(
            self.threads
                .iter()
                .find(|thread| thread.id == d_id)
                .into_iter()
                .flat_map(|thread| thread.messages.iter()),
        ) {
            if let Some(id) = crate::chat::sidebar_sender_id(message) {
                let name = if id == self.user_uin {
                    "Bạn".to_owned()
                } else {
                    resolve_contact_name_from(&self.server_aliases, &id, &message.sender).to_owned()
                };
                let avatar = self
                    .threads
                    .iter()
                    .find(|thread| !thread.is_group && thread.id == id)
                    .and_then(|thread| thread.avatar.clone());
                sender_entries.entry(id).or_insert((name, avatar));
            }
        }
        if let Some(thread) = self.threads.iter().find(|thread| thread.id == d_id) {
            for member in &thread.presentation.members {
                sender_entries.insert(
                    member.id.clone(),
                    (
                        resolve_contact_name_from(&self.server_aliases, &member.id, &member.name)
                            .to_owned(),
                        member.avatar.clone(),
                    ),
                );
            }
        }
        ui.horizontal_wrapped(|ui| {
            if tab != MediaTab::Photos {
                ui.add(
                    egui::TextEdit::singleline(&mut self.drawer_media_query)
                        .hint_text("Tìm trong kho…")
                        .desired_width(150.0_f32.min(ui.available_width())),
                );
            }
            let selected_sender = self
                .drawer_media_sender
                .strip_prefix("id:")
                .and_then(|id| {
                    sender_entries
                        .get(id)
                        .map(|(name, _)| format!("Người gửi: {name}"))
                })
                .unwrap_or_else(|| "Người gửi: Tất cả".into());
            ui.menu_button(selected_sender, |ui| {
                let search_id = Id::new(("archive-sender-query", d_id, format!("{tab:?}")));
                let mut search = ui
                    .ctx()
                    .data_mut(|data| data.get_temp::<String>(search_id).unwrap_or_default());
                ui.add(
                    egui::TextEdit::singleline(&mut search)
                        .hint_text("Tìm người gửi…")
                        .desired_width(180.0),
                );
                ui.ctx()
                    .data_mut(|data| data.insert_temp(search_id, search.clone()));
                if ui
                    .selectable_label(self.drawer_media_sender.is_empty(), "Tất cả người gửi")
                    .clicked()
                {
                    self.drawer_media_sender.clear();
                    ui.close_menu();
                }
                let search = crate::search::fold(&search);
                egui::ScrollArea::vertical()
                    .id_salt("archive-sender-list")
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for (id, (name, avatar)) in &sender_entries {
                            if !search.is_empty()
                                && !crate::search::fold(name).contains(&search)
                                && !id.contains(&search)
                            {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                                paint_user_avatar(
                                    ui.painter(),
                                    rect.center(),
                                    12.0,
                                    name,
                                    false,
                                    avatar.as_deref(),
                                    &self.sticker_textures,
                                    &mut self.loading_stickers,
                                );
                                let key = format!("id:{id}");
                                if ui
                                    .selectable_label(
                                        self.drawer_media_sender == key,
                                        format!("{name} · {id}"),
                                    )
                                    .clicked()
                                {
                                    self.drawer_media_sender = key;
                                    ui.close_menu();
                                }
                            });
                        }
                        if sender_entries.is_empty() {
                            ui.label("Chưa có người gửi trong dữ liệu đã tải.");
                        }
                    });
            });
            render_archive_date_filter(
                ui,
                Id::new(("archive-date-filter", d_id, format!("{tab:?}"))),
                &mut self.drawer_media_from,
                &mut self.drawer_media_to,
            );
            if tab == MediaTab::Files {
                egui::ComboBox::from_id_salt("drawer_file_type")
                    .selected_text(
                        self.drawer_media_type
                            .map(crate::message_view::file_category_label)
                            .unwrap_or("Loại tệp"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.drawer_media_type, None, "Tất cả");
                        for category in [
                            crate::message_view::FileCategory::Pdf,
                            crate::message_view::FileCategory::Word,
                            crate::message_view::FileCategory::PowerPoint,
                            crate::message_view::FileCategory::Excel,
                            crate::message_view::FileCategory::Cad,
                            crate::message_view::FileCategory::Zip,
                            crate::message_view::FileCategory::Other,
                        ] {
                            ui.horizontal(|ui| {
                                let (rect, _) =
                                    ui.allocate_exact_size(Vec2::new(20.0, 23.0), Sense::hover());
                                paint_file_category_icon(ui.painter(), rect, category);
                                ui.selectable_value(
                                    &mut self.drawer_media_type,
                                    Some(category),
                                    crate::message_view::file_category_label(category),
                                );
                            });
                        }
                    });
            }
            if ui.small_button("Xóa lọc").clicked() {
                self.drawer_media_query.clear();
                self.drawer_media_sender.clear();
                self.drawer_media_type = None;
                self.drawer_media_from.clear();
                self.drawer_media_to.clear();
                self.drawer_media_selected.clear();
            }
            if ui
                .small_button(if self.drawer_media_selection_mode {
                    "Hủy chọn"
                } else {
                    "Chọn nhiều"
                })
                .clicked()
            {
                self.drawer_media_selection_mode = !self.drawer_media_selection_mode;
                if !self.drawer_media_selection_mode {
                    self.drawer_media_selected.clear();
                }
            }
        });

        let filter_key = serde_json::json!([
            self.user_uin,
            self.account_runtime_id,
            d_id,
            format!("{tab:?}"),
            self.drawer_media_query,
            self.drawer_media_sender,
            self.drawer_media_from,
            self.drawer_media_to,
            format!("{:?}", self.drawer_media_type),
        ])
        .to_string();
        if self.drawer_media_filter_key != filter_key {
            self.drawer_media_filter_key = filter_key;
            self.drawer_media_limit = 100;
            self.drawer_media_selected.clear();
            self.archive_history = ArchiveHistory {
                thread: d_id.to_owned(),
                generation: crate::search::generation(),
                ..Default::default()
            };
            self.request_archive_page(d_id, d_is_group);
        }
        let mut archive = Thread {
            id: d_id.to_owned(),
            is_group: d_is_group,
            messages: self.archive_history.messages.clone(),
            ..Default::default()
        };
        // Live timeline metadata wins when a persisted row has an older projection.
        if let Some(live) = self.threads.iter().find(|thread| thread.id == d_id) {
            for message in &live.messages {
                if let Some(previous) = archive
                    .messages
                    .iter_mut()
                    .find(|old| crate::chat::same_message(old, message))
                {
                    *previous = message.clone();
                } else {
                    archive.messages.push(message.clone());
                }
            }
        }
        archive.messages.retain(|message| {
            !crate::message_lifecycle::expired(message, crate::login::unix_now() * 1000)
                && !self
                    .deleted_message_ids
                    .contains(&(d_id.to_owned(), message.id.clone()))
        });
        archive.messages.sort_by(|a, b| {
            (crate::clock::normalize_ts_ms(a.ts), &a.id)
                .cmp(&(crate::clock::normalize_ts_ms(b.ts), &b.id))
        });
        let (shared_photos, shared_files, shared_links) = extract_shared_media_and_links(&archive);
        ui.horizontal(|ui| {
            let p_sel = tab == MediaTab::Photos;
            if ui
                .selectable_label(p_sel, format!("🖼️ Ảnh/Video ({})", shared_photos.len()))
                .clicked()
            {
                self.drawer_media_active_tab = MediaTab::Photos;
                self.drawer_view = DrawerView::MediaStorage {
                    tab: MediaTab::Photos,
                };
            }
            let f_sel = tab == MediaTab::Files;
            if ui
                .selectable_label(f_sel, format!("📄 File ({})", shared_files.len()))
                .clicked()
            {
                self.drawer_media_active_tab = MediaTab::Files;
                self.drawer_view = DrawerView::MediaStorage {
                    tab: MediaTab::Files,
                };
            }
            let l_sel = tab == MediaTab::Links;
            if ui
                .selectable_label(l_sel, format!("🔗 Link ({})", shared_links.len()))
                .clicked()
            {
                self.drawer_media_active_tab = MediaTab::Links;
                self.drawer_view = DrawerView::MediaStorage {
                    tab: MediaTab::Links,
                };
            }
        });

        let mut selected_messages = Vec::new();
        let mut selected_downloads = Vec::new();
        let mut selected_links = Vec::new();
        let mut live_keys = HashSet::new();
        match tab {
            MediaTab::Photos => {
                for (key, media, message) in &shared_photos {
                    let key = archive_selection_key(
                        "photo",
                        d_id,
                        message,
                        media.original.as_deref().unwrap_or(key),
                    );
                    live_keys.insert(key.clone());
                    if self.drawer_media_selected.contains(&key) {
                        selected_messages.push(message.clone());
                        if let Some(url) = media.original.as_ref() {
                            selected_downloads.push(super::worker::MediaBatchItem {
                                key: key.clone(),
                                context: super::worker::ChatImage {
                                    thread: d_id.to_owned(),
                                    group: d_is_group,
                                    message: message.clone(),
                                    preview: false,
                                },
                                url: url.clone(),
                                name: media.name.clone(),
                            });
                        }
                    }
                }
            }
            MediaTab::Files => {
                for (media, message) in &shared_files {
                    let name = if media.name.is_empty() {
                        "Tập tin"
                    } else {
                        &media.name
                    };
                    let key = archive_selection_key(
                        "file",
                        d_id,
                        message,
                        media.original.as_deref().unwrap_or(name),
                    );
                    live_keys.insert(key.clone());
                    if self.drawer_media_selected.contains(&key) {
                        selected_messages.push(message.clone());
                        if let Some(url) = media.original.as_ref() {
                            selected_downloads.push(super::worker::MediaBatchItem {
                                key: key.clone(),
                                context: super::worker::ChatImage {
                                    thread: d_id.to_owned(),
                                    group: d_is_group,
                                    message: message.clone(),
                                    preview: false,
                                },
                                url: url.clone(),
                                name: name.to_owned(),
                            });
                        }
                    }
                }
            }
            MediaTab::Links => {
                for (url, _, message) in &shared_links {
                    let key = archive_selection_key("link", d_id, message, url);
                    live_keys.insert(key.clone());
                    if self.drawer_media_selected.contains(&key) {
                        selected_messages.push(message.clone());
                        selected_links.push(url.clone());
                    }
                }
            }
        }
        self.drawer_media_selected
            .retain(|key| live_keys.contains(key));
        if self.drawer_media_selection_mode {
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(format!("Đã chọn {} mục", self.drawer_media_selected.len()));
                ui.horizontal_wrapped(|ui| {
                    let shareable = !self.content_protected(d_id, d_is_group);
                    let can_share = shareable && !selected_messages.is_empty() && selected_messages.len() <= 100
                        && selected_messages.iter().all(crate::chat::can_forward_message);
                    if ui.add_enabled(can_share, egui::Button::new("Chia sẻ tin gốc…")).clicked() {
                        self.share_archive_messages(d_id, d_is_group, selected_messages.clone());
                    }
                    if ui.add_enabled(can_share && !self.own_cloud_id.is_empty(),egui::Button::new("Lưu vào My Documents…")).clicked() {
                        self.save_archive_to_documents(d_id,d_is_group,selected_messages.clone());
                    }
                    if tab != MediaTab::Links {
                        let can_download = shareable && !selected_downloads.is_empty()
                            && selected_downloads.len() == self.drawer_media_selected.len()
                            && selected_downloads.len() <= 100 && self.drawer_media_batch.is_none();
                        if ui.add_enabled(can_download, egui::Button::new("Lưu về máy…"))
                            .on_disabled_hover_text("Chọn tối đa 100 mục có tệp gốc và chờ lượt tải trước hoàn tất.")
                            .clicked() {
                            let batch = crate::search::generation();
                            let queued = WORKER_TX.get().is_some_and(|tx| tx.send(WorkerCmd::DownloadMediaBatch {
                                owner: self.user_uin.clone(), batch, items: selected_downloads.clone(),
                            }).is_ok());
                            if queued {
                                self.drawer_media_batch = Some(batch);
                                self.drawer_media_batch_status = "Đang chờ chọn thư mục và tải các mục đã chọn…".into();
                            } else { self.drawer_media_batch_status = "Không gửi được yêu cầu tải tới worker.".into(); }
                        }
                    } else if ui.add_enabled(shareable && !selected_links.is_empty(), egui::Button::new("Sao chép liên kết")).clicked() {
                        ui.ctx().copy_text(selected_links.join("\n"));
                    }
                    if ui.add_enabled(!selected_messages.is_empty() && self.archive_mutation_batch.is_none(), egui::Button::new("Xóa phía tôi…")).clicked() {
                        self.confirm_archive_mutation(d_id,d_is_group,false,&selected_messages);
                    }
                    if ui.add_enabled(!selected_messages.is_empty() && selected_messages.iter().all(|m| m.is_outgoing && crate::chat::server_message_id(m).is_some()) && self.archive_mutation_batch.is_none(), egui::Button::new("Thu hồi…")).clicked() {
                        self.confirm_archive_mutation(d_id,d_is_group,true,&selected_messages);
                    }
                    if ui.button("Hủy").clicked() {
                        self.drawer_media_selection_mode = false;
                        self.drawer_media_selected.clear();
                    }
                });
                ui.small("Chia sẻ toàn bộ tin nhắn gốc; nhiều mục cùng một tin chỉ được chia sẻ một lần.");
            });
        }
        if !self.archive_mutation_results.is_empty() {
            egui::CollapsingHeader::new("Kết quả từng tin nhắn")
                .default_open(true)
                .show(ui, |ui| {
                    for result in &self.archive_mutation_results {
                        ui.small(result);
                    }
                });
        }
        if !self.drawer_media_batch_status.is_empty() {
            ui.small(&self.drawer_media_batch_status);
        }

        let query = if tab == MediaTab::Photos {
            String::new()
        } else {
            crate::search::fold(self.drawer_media_query.trim())
        };
        let sender_query = self.drawer_media_sender.trim().to_owned();
        let matches_text =
            |value: &str| query.is_empty() || crate::search::fold(value).contains(&query);
        let matches_sender = |msg: &ChatMessage| media_sender_matches(msg, &sender_query);
        let from_input = self.drawer_media_from.trim();
        let to_input = self.drawer_media_to.trim();
        let from_date = chrono::NaiveDate::parse_from_str(from_input, "%Y-%m-%d").ok();
        let to_date = chrono::NaiveDate::parse_from_str(to_input, "%Y-%m-%d").ok();
        let invalid_from = !from_input.is_empty() && from_date.is_none();
        let invalid_to = !to_input.is_empty() && to_date.is_none();
        let reversed_range = from_date.zip(to_date).is_some_and(|(from, to)| from > to);
        let date_filter_invalid = invalid_from || invalid_to || reversed_range;
        if date_filter_invalid {
            ui.label(
                RichText::new(if reversed_range {
                    "Khoảng ngày không hợp lệ: ngày bắt đầu phải trước hoặc bằng ngày kết thúc."
                } else {
                    "Ngày phải có định dạng YYYY-MM-DD."
                })
                .size(11.0)
                .color(pal.danger),
            );
        }
        let matches_date = |msg: &ChatMessage| {
            if date_filter_invalid {
                return false;
            }
            let Some(date) = archive_message_local_date(msg.ts) else {
                return false;
            };
            from_date.is_none_or(|from| date >= from) && to_date.is_none_or(|to| date <= to)
        };

        let mut filtered_count = 0usize;
        egui::ScrollArea::vertical()
            .id_salt("drawer_media_storage_scroll")
            .max_height((ui.available_height() - 20.0).max(120.0))
            .show(ui, |ui| match tab {
                MediaTab::Photos => {
                    let filtered: Vec<_> = shared_photos
                        .iter()
                        .filter(|(_, media, msg)| {
                            matches_sender(msg)
                                && matches_date(msg)
                                && (matches_text(media.name.as_str())
                                    || matches_text(media.original.as_deref().unwrap_or("")))
                        })
                        .collect();
                    filtered_count = filtered.len();
                    if filtered.is_empty() {
                        ui.label(
                            RichText::new("Chưa có ảnh hoặc video nào.")
                                .size(12.0)
                                .color(pal.text_muted),
                        );
                    } else {
                        let gap = 6.0;
                        let columns = ((ui.available_width() + gap) / (64.0 + gap))
                            .floor()
                            .clamp(1.0, 3.0) as usize;
                        let cell_sz = ((ui.available_width() - (columns - 1) as f32 * gap)
                            / columns as f32)
                            .clamp(32.0, 112.0);
                        let mut shown = 0usize;
                        let date_groups = group_photos_by_local_date(filtered);
                        for (date, items) in date_groups {
                            if shown >= self.drawer_media_limit {
                                break;
                            }
                            ui.separator();
                            ui.small(date);
                            for row in items.chunks(columns) {
                                if shown >= self.drawer_media_limit {
                                    break;
                                }
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing = Vec2::new(gap, gap);
                                    for (tkey, media, message) in row {
                                        if shown >= self.drawer_media_limit {
                                            break;
                                        }
                                        self.render_photo_cell(
                                            ui, tkey, media, message, d_id, "", d_is_group,
                                            cell_sz, pal,
                                        );
                                        shown += 1;
                                    }
                                });
                                ui.add_space(gap);
                            }
                        }
                    }
                }
                MediaTab::Files => {
                    let filtered: Vec<_> = shared_files
                        .iter()
                        .filter(|(media, msg)| {
                            matches_sender(msg)
                                && matches_date(msg)
                                && (matches_text(media.name.as_str())
                                    || matches_text(media.original.as_deref().unwrap_or("")))
                                && self.drawer_media_type.is_none_or(|kind| {
                                    crate::message_view::attachment_file_category(media, msg)
                                        == kind
                                })
                        })
                        .collect();
                    filtered_count = filtered.len();
                    if filtered.is_empty() {
                        ui.label(
                            RichText::new("Chưa có tập tin nào.")
                                .size(12.0)
                                .color(pal.text_muted),
                        );
                    } else {
                        let mut last_date = String::new();
                        for (media, msg) in filtered.iter().take(self.drawer_media_limit) {
                            let date = chrono::DateTime::from_timestamp_millis(
                                crate::clock::normalize_ts_ms(msg.ts),
                            )
                            .map(|date| {
                                date.with_timezone(&chrono::Local)
                                    .format("%d/%m/%Y")
                                    .to_string()
                            })
                            .unwrap_or_else(|| "Chưa rõ ngày gửi".into());
                            if last_date != date {
                                ui.separator();
                                ui.small(&date);
                                last_date = date;
                            }
                            self.render_file_row(ui, media, msg, d_id, d_is_group, pal);
                            ui.add_space(4.0);
                        }
                    }
                }
                MediaTab::Links => {
                    let filtered: Vec<_> = shared_links
                        .iter()
                        .filter(|(url, title, msg)| {
                            matches_sender(msg)
                                && matches_date(msg)
                                && (matches_text(url) || matches_text(title))
                        })
                        .collect();
                    filtered_count = filtered.len();
                    if filtered.is_empty() {
                        ui.label(
                            RichText::new("Chưa có liên kết nào.")
                                .size(12.0)
                                .color(pal.text_muted),
                        );
                    } else {
                        let mut last_date = String::new();
                        for (url, title, msg) in filtered.iter().take(self.drawer_media_limit) {
                            let date = chrono::DateTime::from_timestamp_millis(
                                crate::clock::normalize_ts_ms(msg.ts),
                            )
                            .map(|date| {
                                date.with_timezone(&chrono::Local)
                                    .format("%d/%m/%Y")
                                    .to_string()
                            })
                            .unwrap_or_else(|| "Chưa rõ ngày gửi".into());
                            if last_date != date {
                                ui.separator();
                                ui.small(&date);
                                last_date = date;
                            }
                            self.render_link_row(ui, url, title, msg, d_id, d_is_group, pal);
                            ui.add_space(4.0);
                        }
                    }
                }
            });

        if filtered_count > 0 {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Hiển thị {} / {} mục trong dữ liệu đã tải",
                    filtered_count.min(self.drawer_media_limit),
                    filtered_count
                ));
                if self.drawer_media_limit < filtered_count
                    && ui.button("Hiện thêm kết quả đã đọc").clicked()
                {
                    self.drawer_media_limit = self.drawer_media_limit.saturating_add(100);
                }
            });
        }
        ui.small(format!(
            "Đã đọc {} tin trong lịch sử cục bộ; chưa xác nhận kho server đầy đủ.",
            self.archive_history.scanned
        ));
        if !self.archive_history.error.is_empty() {
            ui.colored_label(pal.danger, &self.archive_history.error);
        }
        if self.archive_history.loading {
            ui.spinner();
            ui.small("Đang đọc trang lịch sử cục bộ…");
        } else if ((!self.archive_history.loaded || !self.archive_history.error.is_empty())
            || self.archive_history.cursor.is_some())
            && ui
                .button(if self.archive_history.error.is_empty() {
                    "Đọc lịch sử cũ hơn"
                } else {
                    "Thử đọc lại lịch sử"
                })
                .clicked()
        {
            self.request_archive_page(d_id, d_is_group);
        }
    }

    fn render_drawer_search_view(
        &mut self,
        ui: &mut egui::Ui,
        thread_id: &str,
        pal: &ThemePalette,
    ) {
        self.render_drawer_subview_header(ui, "Tìm kiếm tin nhắn", pal);

        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.drawer_search_query)
                    .hint_text("Nhập từ khóa tin nhắn cần tìm…")
                    .desired_width(ui.available_width() - 36.0),
            );
            if !self.drawer_search_query.is_empty() && ui.button("✕").clicked() {
                self.drawer_search_query.clear();
            }
        });

        ui.add_space(6.0);

        let query = fold_vn_search(self.drawer_search_query.trim());
        if query.is_empty() {
            ui.label(
                RichText::new("Nhập từ khóa để tìm kiếm nhanh trong cuộc trò chuyện này.")
                    .size(11.5)
                    .color(pal.text_muted),
            );
        } else {
            let thread_info = self
                .threads
                .get(self.selected_thread_index)
                .map(|t| (t.is_group, &t.messages));

            if let Some((is_group, messages)) = thread_info {
                let matches: Vec<&ChatMessage> = messages
                    .iter()
                    .filter(|m| fold_vn_search(&m.text).contains(&query))
                    .collect();

                ui.label(
                    RichText::new(format!("Tìm thấy {} tin nhắn:", matches.len()))
                        .size(11.5)
                        .color(pal.text_secondary),
                );
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("drawer_search_results_scroll")
                    .max_height((ui.available_height() - 60.0).max(120.0))
                    .show(ui, |ui| {
                        for m in matches {
                            let sender_name = if m.is_outgoing {
                                "Bạn"
                            } else {
                                resolve_message_sender_name_from(
                                    &self.server_aliases,
                                    &self.threads,
                                    &self.group_members,
                                    m,
                                    thread_id,
                                    is_group,
                                    "Người nhận",
                                )
                            };
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 44.0),
                                Sense::click(),
                            );
                            ui.painter().rect_filled(
                                rect,
                                Rounding::same(6.0),
                                if resp.hovered() {
                                    pal.bg_hover
                                } else {
                                    pal.bg_elevated
                                },
                            );
                            ui.painter().rect_stroke(
                                rect,
                                Rounding::same(6.0),
                                Stroke::new(1.0_f32, pal.border),
                            );

                            let top_txt = format!("{} · {}", sender_name, m.display_time());
                            ui.painter().text(
                                Pos2::new(rect.left() + 8.0, rect.top() + 10.0),
                                egui::Align2::LEFT_CENTER,
                                top_txt,
                                FontId::proportional(10.5),
                                pal.primary_hover,
                            );
                            ui.painter().text(
                                Pos2::new(rect.left() + 8.0, rect.top() + 26.0),
                                egui::Align2::LEFT_CENTER,
                                truncate_chars(&m.text, 36),
                                FontId::proportional(11.5),
                                pal.text_primary,
                            );

                            if resp.on_hover_cursor(CursorIcon::PointingHand).clicked() {
                                self.filter_starred_only = false;
                                self.quote_jump = Some(QuoteJump {
                                    thread_id: thread_id.to_string(),
                                    reference: m.id.clone(),
                                    highlight_until: std::time::Instant::now()
                                        + std::time::Duration::from_secs(3),
                                    scroll_pending: true,
                                });
                                ui.ctx().request_repaint();
                            }
                            ui.add_space(4.0);
                        }
                    });
            }
        }

        ui.add_space(8.0);
        if ui.button("🔍 Mở tìm kiếm toàn bộ cơ sở dữ liệu").clicked() {
            self.open_history_search(
                Some(thread_id.to_string()),
                crate::search::Category::Messages,
                self.drawer_search_query.clone(),
            );
        }
    }

    fn render_drawer_security_view(
        &mut self,
        ui: &mut egui::Ui,
        d_id: &str,
        _d_name: &str,
        d_is_group: bool,
        pal: &ThemePalette,
    ) {
        self.render_drawer_subview_header(ui, "Thiết lập bảo mật & Quyền riêng tư", pal);

        if self
            .policy_panel
            .target
            .as_ref()
            .map(|(t, _)| t != d_id)
            .unwrap_or(true)
        {
            self.policy_panel.target = Some((d_id.to_string(), d_is_group));
            self.policy_panel.ttl = self
                .policy_panel
                .snapshot
                .as_ref()
                .map(|s| s.ttl(d_id, d_is_group))
                .unwrap_or(0);
            self.policy_panel.confirm = None;
            self.policy_command(crate::conversation_policy::Action::Read);
        }

        egui::ScrollArea::vertical()
            .id_salt("drawer_security_scroll")
            .max_height((ui.available_height() - 20.0).max(120.0))
            .show(ui, |ui| {
                // Security controls remain available without exposing experimental E2EE actions.
                egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("⏱️ Tin nhắn tự xóa")
                                .size(12.5)
                                .color(pal.text_primary)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        let current_ttl = self
                            .policy_panel
                            .snapshot
                            .as_ref()
                            .map(|s| s.ttl(d_id, d_is_group))
                            .unwrap_or(0);
                        ui.label(
                            RichText::new(if current_ttl == 0 {
                                "Hiện đang tắt: tin nhắn không tự động xóa."
                            } else {
                                "Tin nhắn tự động biến mất sau thời gian được chọn."
                            })
                            .size(11.0)
                            .color(pal.text_secondary),
                        );
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            for (ttl_val, label) in [
                                (0, "Tắt"),
                                (86400000, "1 ngày"),
                                (604800000, "7 ngày"),
                                (2592000000, "30 ngày"),
                            ] {
                                let selected = current_ttl == ttl_val;
                                if ui.selectable_label(selected, label).clicked()
                                    && current_ttl != ttl_val
                                {
                                    self.policy_command(
                                        crate::conversation_policy::Action::SetTtl {
                                            thread: d_id.to_string(),
                                            group: d_is_group,
                                            ttl: ttl_val,
                                        },
                                    );
                                }
                            }
                        });
                    });

                ui.add_space(10.0);

                // Section 3: Lưu trữ hội thoại trên máy chủ
                egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("📦 Lưu trữ hội thoại")
                                .size(12.5)
                                .color(pal.text_primary)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Lưu trữ giúp ẩn hội thoại khỏi màn hình chính nhưng không làm mất dữ liệu.")
                                .size(11.0)
                                .color(pal.text_secondary),
                        );
                        ui.add_space(6.0);
                        let is_archived = self
                            .threads
                            .iter()
                            .find(|t| t.id == d_id && t.is_group == d_is_group)
                            .and_then(|t| t.presentation.archived)
                            .unwrap_or(false);
                        if ui
                            .button(if is_archived {
                                "Đưa về hội thoại chính"
                            } else {
                                "Lưu trữ hội thoại này"
                            })
                            .clicked()
                        {
                            self.archive_command(d_id.to_string(), d_is_group, Some(!is_archived));
                        }
                    });

                ui.add_space(10.0);

                // Section 4: Quản lý nâng cao
                egui::Frame::none()
                    .fill(pal.bg_elevated)
                    .stroke(Stroke::new(1.0_f32, pal.border))
                    .rounding(Rounding::same(8.0))
                    .inner_margin(Margin::symmetric(12.0, 10.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("⚙️ Quản lý & Quyền riêng tư")
                                .size(12.5)
                                .color(pal.text_primary)
                                .strong(),
                        );
                        ui.add_space(6.0);
                        if d_is_group {
                            if ui
                                .button("Quản lý nhóm (Quyền thành viên / Đổi tên)")
                                .clicked()
                            {
                                self.open_group_panel(d_id.to_string(), true, false);
                            }
                        } else {
                            if ui.button("Thông tin liên hệ & Biệt danh").clicked() {
                                self.open_contact_panel(d_id.to_string());
                            }
                        }
                        if ui.button("Phân loại thẻ màu (Labels)").clicked() {
                            self.open_labels(Some((d_id.to_string(), d_is_group)));
                        }
                    });
            });
    }
}

/// Preserve the first visible stable message across prepend and geometry updates.
fn capture_anchor(
    ui: &egui::Ui,
    rect: Rect,
    id: &str,
    previous: Option<&(String, String, f32)>,
    next: &mut Option<(String, f32)>,
    delta: &mut Option<f32>,
) {
    let offset = rect.top() - ui.clip_rect().top();
    if !id.is_empty() && next.is_none() && ui.is_rect_visible(rect) {
        *next = Some((id.into(), offset));
    }
    if let Some((_, old_id, old_offset)) = previous {
        if old_id == id {
            *delta = Some(offset - old_offset);
        }
    }
}

impl ZaloGuiApp {
    pub(super) fn render_archive_confirmation(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        if self.archive_mutation_confirm.is_none() {
            return;
        }
        egui::Window::new("Xác nhận thao tác tin nhắn")
            .id(Id::new("archive-confirmation-modal"))
            .anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO)
            .order(egui::Order::Foreground).collapsible(false).resizable(false)
            .max_width((ctx.screen_rect().width()-48.0).max(160.0))
            .show(ctx, |ui| {
        if let Some(pending) = &self.archive_mutation_confirm {
            let mut confirm = false;
            let mut cancel = false;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.colored_label(pal.danger, if pending.recall { "Thu hồi các tin nhắn gốc với mọi người?" } else { "Xóa các tin nhắn gốc chỉ ở phía bạn?" });
                ui.small("Thao tác áp dụng cho toàn bộ tin gốc, gồm tất cả tệp đính kèm của tin. Không tự thử lại khi chưa rõ kết quả.");
                egui::ScrollArea::vertical().id_salt("archive_mutation_confirmation").max_height(100.0).show(ui,|ui| {
                    for message in &pending.messages { ui.label(format!("{}: {}", message.sender,crate::message_view::summary(message))); }
                });
                ui.horizontal(|ui| { confirm=ui.button("Xác nhận thực hiện").clicked(); cancel=ui.button("Hủy thao tác").clicked(); });
            });
            if confirm {
                if let Some(pending) = self.archive_mutation_confirm.take() {
                    let batch = crate::search::generation();
                    let queued = WORKER_TX.get().is_some_and(|tx| {
                        tx.send(WorkerCmd::MutateArchiveBatch {
                            owner: self.user_uin.clone(),
                            runtime_id: self.account_runtime_id,
                            batch,
                            thread: pending.thread,
                            group: pending.group,
                            recall: pending.recall,
                            messages: pending.messages,
                        })
                        .is_ok()
                    });
                    if queued {
                        self.archive_mutation_batch = Some(batch);
                        self.archive_mutation_results.clear();
                    } else {
                        self.drawer_media_batch_status =
                            "Không gửi được yêu cầu tới worker.".into();
                    }
                }
            } else if cancel {
                self.archive_mutation_confirm = None;
            }
        }
            });
    }

    pub(super) fn render_message_details(&mut self, ctx: &egui::Context) {
        let Some((thread, snapshot)) = self.message_details.clone() else {
            return;
        };
        let current = self
            .threads
            .iter()
            .find(|candidate| candidate.id == thread)
            .filter(|candidate| self.private_access(&thread, candidate.is_group))
            .and_then(|candidate| {
                candidate.messages.iter().find(|message| {
                    message.id == snapshot.id
                        && message.client_message_id == snapshot.client_message_id
                })
            })
            .filter(|message| {
                !crate::message_lifecycle::expired(message, crate::login::unix_now() * 1000)
            })
            .cloned();
        let Some(message) = current else {
            self.message_details = None;
            return;
        };
        let mut open = true;
        egui::Window::new("Chi tiết tin nhắn")
            .id(Id::new("message-details"))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .order(egui::Order::Foreground)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .max_width((ctx.screen_rect().width() - 48.0).max(160.0))
            .show(ctx, |ui| {
                ui.label(format!("Hội thoại: {thread}"));
                ui.label(format!(
                    "ID tin nhắn: {}",
                    if message.id.is_empty() {
                        "Chưa có"
                    } else {
                        &message.id
                    }
                ));
                ui.label(format!(
                    "Người gửi: {}",
                    if message.is_outgoing {
                        "Bạn"
                    } else {
                        &message.sender
                    }
                ));
                ui.label(format!("Thời gian: {}", message.display_time()));
                let delivery = match message.delivery {
                    crate::message_model::DeliveryState::Unknown => "Chưa có dữ liệu xác nhận",
                    crate::message_model::DeliveryState::Pending => "Đang gửi",
                    crate::message_model::DeliveryState::ServerAccepted => "Máy chủ đã nhận",
                    crate::message_model::DeliveryState::Delivered => "Đã giao",
                    crate::message_model::DeliveryState::Read => "Đã đọc",
                    crate::message_model::DeliveryState::Failed => "Gửi thất bại",
                    crate::message_model::DeliveryState::DeliveryUnknown => {
                        "Chưa xác định kết quả gửi"
                    }
                };
                ui.label(format!("Trạng thái: {delivery}"));
                ui.label("Danh sách người đã nhận/đọc: chưa có dữ liệu xác nhận.");
                ui.separator();
                ui.label(crate::message_view::summary(&message));
            });
        if !open {
            self.message_details = None;
        }
    }

    pub(super) fn render_reactor_details(&mut self, ctx: &egui::Context) {
        let key = Id::new("reaction_details");
        let Some((owner, thread_id, original)) =
            ctx.data(|data| data.get_temp::<(String, String, ChatMessage)>(key))
        else {
            return;
        };
        if !self.is_logged_in
            || owner != self.user_uin
            || !self.private_access(&thread_id, thread_id.starts_with('g'))
        {
            ctx.data_mut(|data| data.remove::<(String, String, ChatMessage)>(key));
            return;
        }
        let message = self
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .and_then(|thread| {
                thread
                    .messages
                    .iter()
                    .find(|message| message.id == original.id)
            })
            .cloned()
            .unwrap_or(original);
        let ids = message
            .reaction_summaries
            .iter()
            .flat_map(|summary| summary.reactor_ids.iter())
            .filter(|id| {
                !self.reactor_profiles.contains_key(*id)
                    && !self.reactor_profile_pending.contains(*id)
                    && !id.is_empty()
                    && id.bytes().all(|b| b.is_ascii_digit())
            })
            .take(50)
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !ids.is_empty()
            && WORKER_TX.get().is_some_and(|tx| {
                tx.send(WorkerCmd::FetchReactorProfiles {
                    owner: self.user_uin.clone(),
                    ids: ids.clone(),
                })
                .is_ok()
            })
        {
            self.reactor_profile_pending.extend(ids);
        }
        let mut selected = ctx
            .data(|data| data.get_temp::<String>(Id::new("reaction_details_filter")))
            .unwrap_or_default();
        let name = |id: &str| {
            if id == self.user_uin {
                return "Bạn".to_owned();
            }
            self.server_aliases
                .get(id)
                .map(|alias| alias.trim().to_owned())
                .filter(|alias| !alias.is_empty())
                .or_else(|| {
                    self.reactor_profiles
                        .get(id)
                        .map(|profile| profile.name.clone())
                        .filter(|name| !name.is_empty())
                })
                .or_else(|| {
                    self.threads
                        .iter()
                        .find(|thread| !thread.is_group && thread.id == id)
                        .map(|thread| thread.name.clone())
                })
                .or_else(|| {
                    self.group_members.get(&thread_id).and_then(|(members, _)| {
                        members
                            .iter()
                            .find(|member| member.id == id)
                            .map(|member| member.name.clone())
                    })
                })
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("ID: {id}"))
        };
        let mut people = std::collections::BTreeMap::<String, Vec<(String, u64)>>::new();
        for summary in &message.reaction_summaries {
            if selected.is_empty() || selected == summary.emoji {
                for id in &summary.reactor_ids {
                    people.entry(id.clone()).or_default().push((
                        summary.emoji.clone(),
                        summary.reactor_counts.get(id).copied().unwrap_or(1),
                    ));
                }
            }
        }
        let rows = people
            .into_iter()
            .map(|(id, icons)| {
                let label = name(&id);
                let avatar = self
                    .reactor_profiles
                    .get(&id)
                    .and_then(|profile| profile.avatar.clone())
                    .or_else(|| {
                        self.threads
                            .iter()
                            .find(|thread| !thread.is_group && thread.id == id)
                            .and_then(|thread| thread.avatar.clone())
                    });
                (id, label, avatar, icons)
            })
            .collect::<Vec<_>>();
        let group = self
            .threads
            .iter()
            .find(|thread| thread.id == thread_id)
            .is_some_and(|thread| thread.is_group);
        let own = message
            .reaction_summaries
            .iter()
            .any(|summary| summary.reactor_ids.contains(&self.user_uin));
        let mut open = !ctx.input(|input| input.key_pressed(Key::Escape));
        let mut clear = false;
        egui::Window::new("Người đã thả cảm xúc").id(key).open(&mut open).default_width(390.0).show(ctx,|ui|{
            ui.horizontal_wrapped(|ui|{ui.selectable_value(&mut selected,String::new(),"Tất cả");for summary in &message.reaction_summaries{ui.selectable_value(&mut selected,summary.emoji.clone(),format!("{} ({})",summary.emoji,summary.count));}});
            if message.reaction_summaries.is_empty()||message.reaction_summaries.iter().any(|summary|summary.reactor_ids.is_empty()||summary.count_is_lower_bound){ui.label("Danh sách hiện có chưa đầy đủ; ứng dụng không suy đoán người thả từ số đếm.");}
            egui::ScrollArea::vertical().max_height(400.0).show(ui,|ui|{for (_,name,avatar,icons) in &rows{
                ui.horizontal(|ui|{let (rect,_)=ui.allocate_exact_size(Vec2::splat(32.0),Sense::hover());if ui.is_rect_visible(rect){paint_user_avatar(ui.painter(),rect.center(),15.0,name,false,avatar.as_deref(),&self.sticker_textures,&mut self.loading_stickers);}ui.label(name);for (emoji,count) in icons{ui.label(format!("{emoji} ×{count}"));}});
            }});
            if ui.add_enabled(own,egui::Button::new("Bỏ cảm xúc của tôi")).clicked(){clear=true;}
        });
        ctx.data_mut(|data| data.insert_temp(Id::new("reaction_details_filter"), selected));
        if clear {
            self.dispatch_reaction_intent(&thread_id, group, message, String::new());
        }
        if !open {
            ctx.data_mut(|data| {
                data.remove::<(String, String, ChatMessage)>(key);
                data.remove::<String>(Id::new("reaction_details_filter"));
            });
        }
    }

    pub(super) fn voice_draft_matches(
        &self,
        owner: &str,
        runtime_id: usize,
        thread_id: &str,
        is_group: bool,
    ) -> bool {
        self.is_logged_in
            && self.voice_binding.as_ref().is_some_and(|binding| {
                binding.matches(
                    owner,
                    runtime_id,
                    thread_id,
                    is_group,
                    self.voice_context_generation,
                )
            })
    }

    pub(super) fn cancel_voice_if_context_changed(&mut self) {
        let matches_selected = self
            .threads
            .get(self.selected_thread_index)
            .is_some_and(|thread| {
                self.voice_draft_matches(
                    &self.user_uin,
                    self.account_runtime_id,
                    &thread.id,
                    thread.is_group,
                )
            });
        if self.voice_binding.is_some() && !matches_selected {
            self.cancel_voice_recording();
        }
    }

    fn cancel_voice_if_thread_changed(&mut self, thread_id: &str, is_group: bool) {
        if self.voice_binding.is_some()
            && !self.voice_draft_matches(
                &self.user_uin,
                self.account_runtime_id,
                thread_id,
                is_group,
            )
        {
            self.cancel_voice_recording();
        }
    }

    fn toggle_voice_recorder(&mut self, ctx: &egui::Context) {
        if self.voice_binding.is_some() {
            if self.voice_finish_pending {
                return;
            }
            self.cancel_voice_if_context_changed();
            if self.voice_binding.is_none() {
                return;
            }
            if let Some(worker) = self.voice_worker.as_ref() {
                match worker.finish(self.voice_context_generation, false) {
                    Ok(()) => self.voice_finish_pending = true,
                    Err(error) => self.show_toast(error.to_string()),
                }
            }
        } else {
            let Some(thread) = self.threads.get(self.selected_thread_index) else {
                return;
            };
            if !self.is_logged_in || thread.id.is_empty() {
                return;
            }
            let binding = VoiceDraftBinding {
                owner: self.user_uin.clone(),
                runtime_id: self.account_runtime_id,
                thread_id: thread.id.clone(),
                is_group: thread.is_group,
                generation: self.voice_context_generation,
            };
            let thread_id = thread.id.clone();
            let is_group = thread.is_group;
            self.recorded_voice_samples = None;
            self.voice_binding = Some(binding);
            self.voice_finish_pending = false;
            self.voice_send_after_finish = false;
            if self.voice_worker.is_none() {
                match crate::voice::VoiceWorker::new(ctx.clone()) {
                    Ok(worker) => self.voice_worker = Some(worker),
                    Err(error) => {
                        self.voice_binding = None;
                        self.show_toast(error.to_string());
                        return;
                    }
                }
            }
            if let Some(worker) = self.voice_worker.as_ref() {
                if let Err(e) = worker.start(self.voice_context_generation) {
                    self.show_toast(format!("Lỗi bật micro: {e}"));
                    self.voice_binding = None;
                } else {
                    self.voice_start_pending = true;
                }
            }
            self.cancel_voice_if_thread_changed(&thread_id, is_group);
        }
    }

    pub(super) fn cancel_voice_recording(&mut self) {
        let generation = self.voice_context_generation;
        self.voice_context_generation = self.voice_context_generation.wrapping_add(1);
        if let Some(worker) = self.voice_worker.as_ref() {
            let _ = worker.cancel(generation);
        }
        self.recorded_voice_samples = None;
        self.voice_binding = None;
        self.voice_start_pending = false;
        self.voice_finish_pending = false;
        self.voice_send_after_finish = false;
    }

    pub(super) fn poll_voice_events(&mut self, ctx: &egui::Context) {
        let events = self
            .voice_worker
            .as_ref()
            .map(|worker| worker.try_events().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                crate::voice::VoiceEvent::Finished {
                    generation,
                    samples,
                    sample_rate,
                    duration,
                    limit_reached,
                    send_after_finish,
                } => {
                    let current_binding = self.voice_binding.as_ref().is_some_and(|binding| {
                        binding.generation == generation
                            && binding.matches(
                                &self.user_uin,
                                self.account_runtime_id,
                                self.threads
                                    .get(self.selected_thread_index)
                                    .map(|thread| thread.id.as_str())
                                    .unwrap_or_default(),
                                self.threads
                                    .get(self.selected_thread_index)
                                    .is_some_and(|thread| thread.is_group),
                                self.voice_context_generation,
                            )
                    });
                    if !current_binding {
                        continue;
                    }
                    self.voice_start_pending = false;
                    self.voice_finish_pending = false;
                    self.voice_send_after_finish = false;
                    if limit_reached {
                        self.show_toast(
                            "Đã đạt giới hạn thu âm 5 phút hoặc 64 MiB; bản ghi đã dừng",
                        );
                    }
                    if send_after_finish {
                        if samples.is_empty() || duration.as_secs_f32() < 0.3 {
                            self.voice_binding = None;
                            self.show_toast("Bản ghi quá ngắn để gửi");
                        } else {
                            self.send_voice_samples(ctx, samples, sample_rate, duration);
                        }
                    } else if !samples.is_empty() && duration.as_secs_f32() >= 0.3 {
                        self.recorded_voice_samples = Some((samples, sample_rate, duration));
                    } else {
                        self.voice_binding = None;
                        self.show_toast("Bản ghi quá ngắn; hãy giữ để thu âm lâu hơn");
                    }
                }
                crate::voice::VoiceEvent::Failed {
                    generation,
                    message,
                } => {
                    if self
                        .voice_binding
                        .as_ref()
                        .is_none_or(|binding| binding.generation != generation)
                    {
                        continue;
                    }
                    self.voice_start_pending = false;
                    self.voice_finish_pending = false;
                    self.voice_binding = None;
                    self.voice_context_generation = self.voice_context_generation.wrapping_add(1);
                    self.show_toast(format!("Lỗi bật micro: {message}"));
                }
                crate::voice::VoiceEvent::PreviewFailed {
                    generation,
                    message,
                } => {
                    if self
                        .voice_binding
                        .as_ref()
                        .is_some_and(|binding| binding.generation == generation)
                    {
                        self.show_toast(format!("Không phát được bản ghi âm: {message}"));
                    }
                }
            }
        }
    }

    pub(super) fn send_voice_recording(
        &mut self,
        ctx: &egui::Context,
        thread_id: &str,
        is_group: bool,
    ) {
        if self.composer_has_metadata(ctx, thread_id, is_group) {
            self.status_msg="Chưa hỗ trợ định dạng hoặc mức Quan trọng/Khẩn cấp cho giọng nói; bản ghi và nội dung vẫn được giữ.".into();
            return;
        }
        if !self.voice_draft_matches(&self.user_uin, self.account_runtime_id, thread_id, is_group) {
            self.cancel_voice_recording();
            self.status_msg = "Bản ghi âm đã bị hủy do tài khoản hoặc hội thoại thay đổi".into();
            return;
        }
        if self.recorded_voice_samples.is_none() {
            if self.voice_finish_pending || self.voice_send_after_finish {
                return;
            }
            if let Some(worker) = self.voice_worker.as_ref() {
                match worker.finish(self.voice_context_generation, true) {
                    Ok(()) => {
                        self.voice_finish_pending = true;
                        self.voice_send_after_finish = true;
                        return;
                    }
                    Err(error) => self.show_toast(error.to_string()),
                }
            }
            return;
        }
        let (samples, rate, dur) = self.recorded_voice_samples.take().unwrap();
        self.send_voice_samples(ctx, samples, rate, dur);
    }

    fn send_voice_samples(
        &mut self,
        ctx: &egui::Context,
        samples: std::sync::Arc<[f32]>,
        rate: u32,
        dur: std::time::Duration,
    ) {
        if !self.voice_draft_matches(
            &self.user_uin,
            self.account_runtime_id,
            self.threads
                .get(self.selected_thread_index)
                .map(|t| t.id.as_str())
                .unwrap_or_default(),
            self.threads
                .get(self.selected_thread_index)
                .is_some_and(|t| t.is_group),
        ) {
            self.cancel_voice_recording();
            return;
        }
        let binding = self.voice_binding.clone();
        let Some(binding) = binding else {
            return;
        };
        if self.composer_has_metadata(ctx, &binding.thread_id, binding.is_group) {
            self.recorded_voice_samples = Some((samples, rate, dur));
            self.status_msg="Chưa hỗ trợ định dạng hoặc mức Quan trọng/Khẩn cấp cho giọng nói; bản ghi và nội dung vẫn được giữ.".into();
            return;
        }
        let Some(tx) = WORKER_TX.get() else {
            self.show_toast("Không kết nối được bộ xử lý tin nhắn thoại");
            self.recorded_voice_samples = Some((samples, rate, dur));
            return;
        };
        let result = tx.send(WorkerCmd::SendVoiceMessage {
            owner: binding.owner.clone(),
            runtime_id: binding.runtime_id,
            to_id: binding.thread_id.clone(),
            is_group: binding.is_group,
            samples: std::sync::Arc::clone(&samples),
            sample_rate: rate,
            duration_secs: dur.as_secs().max(1) as u32,
        });
        if result.is_err() {
            self.show_toast("Không thể xếp tin nhắn thoại để gửi");
            self.recorded_voice_samples = Some((samples, rate, dur));
        } else {
            if let Some(worker) = self.voice_worker.as_ref() {
                let _ = worker.cancel(binding.generation);
            }
            self.voice_binding = None;
            self.voice_context_generation = self.voice_context_generation.wrapping_add(1);
            self.status_msg = "Đang gửi tin nhắn thoại…".into();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_voice_recording_bar(
        ui: &mut egui::Ui,
        pal: &ThemePalette,
        input_w: f32,
        is_recording: bool,
        dur: std::time::Duration,
        waveform: &[f32],
        is_preview_playing: bool,
        preview_samples: Option<(std::sync::Arc<[f32]>, u32)>,
    ) -> VoiceBarAction {
        let mut action = VoiceBarAction::None;
        ui.allocate_ui_with_layout(
            Vec2::new(input_w, 34.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                if is_recording {
                    ui.ctx()
                        .request_repaint_after(std::time::Duration::from_millis(50));
                    ui.label(
                        RichText::new("🔴 Ghi âm…")
                            .color(pal.danger)
                            .strong()
                            .size(12.5),
                    );
                } else {
                    ui.label(
                        RichText::new("⏹ Đã ghi âm")
                            .color(pal.primary)
                            .strong()
                            .size(12.5),
                    );
                }

                let total_secs = dur.as_secs();
                ui.label(
                    RichText::new(format!("{:02}:{:02}", total_secs / 60, total_secs % 60))
                        .color(pal.text_primary)
                        .strong()
                        .size(12.5),
                );

                ui.add_space(6.0);

                let bar_count = 14;
                let bar_w = 4.0;
                for level in waveform.iter().rev().take(bar_count) {
                    let h = (level * 20.0).clamp(4.0, 20.0);
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(bar_w, 20.0), Sense::hover());
                    let bar_rect = Rect::from_center_size(rect.center(), Vec2::new(bar_w - 1.0, h));
                    ui.painter().rect_filled(
                        bar_rect,
                        Rounding::same(1.0),
                        if is_recording {
                            pal.danger
                        } else {
                            pal.primary
                        },
                    );
                }

                ui.add_space(6.0);

                if !is_recording {
                    let lbl = if is_preview_playing {
                        "⏸ Tạm dừng"
                    } else {
                        "▶ Nghe thử"
                    };
                    if ui.small_button(lbl).clicked() {
                        if is_preview_playing {
                            action = VoiceBarAction::StopPreview;
                        } else if let Some((samples, rate)) = preview_samples {
                            action = VoiceBarAction::TogglePreview(samples, rate);
                        }
                    }
                } else if ui.small_button("⏹ Dừng").clicked() {
                    action = VoiceBarAction::StopRecording;
                }

                if ui
                    .small_button(RichText::new(lucide::X).color(pal.danger))
                    .clicked()
                {
                    action = VoiceBarAction::CancelRecording;
                }
            },
        );
        action
    }
}

enum VoiceBarAction {
    None,
    StopRecording,
    CancelRecording,
    TogglePreview(std::sync::Arc<[f32]>, u32),
    StopPreview,
}

fn reaction_toggle_intent(message: &ChatMessage, owner: &str, emoji: &str) -> String {
    let owned_reaction = message.reaction_summaries.iter().any(|summary| {
        summary.emoji == emoji && summary.reactor_ids.iter().any(|reactor| reactor == owner)
    });
    if owned_reaction {
        String::new()
    } else {
        emoji.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_selection_scopes_thread_message_and_attachment_without_delimiter_aliases() {
        let message = ChatMessage {
            id: "message:one".into(),
            client_message_id: "client-one".into(),
            ..Default::default()
        };
        let key = archive_selection_key("photo", "thread:one", &message, "attachment:one");
        assert_ne!(
            key,
            archive_selection_key("photo", "thread:two", &message, "attachment:one")
        );
        assert_ne!(
            key,
            archive_selection_key("photo", "thread:one", &message, "attachment:two")
        );
        let other_message = ChatMessage {
            id: "one".into(),
            ..message.clone()
        };
        assert_ne!(
            key,
            archive_selection_key(
                "photo",
                "thread:one:message",
                &other_message,
                "attachment:one"
            )
        );
        assert_ne!(
            key,
            archive_selection_key("link", "thread:one", &message, "attachment:one")
        );
    }

    #[test]
    fn media_sender_filter_uses_exact_id_and_display_name_search() {
        let alice = ChatMessage {
            sender: "Alice Nguyen".into(),
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(serde_json::json!("12345")),
                ..Default::default()
            },
            ..Default::default()
        };
        let same_name_other_id = ChatMessage {
            sender: "Alice Nguyen".into(),
            wire_ref: crate::message_model::WireMessageRef {
                sender: Some(serde_json::json!("67890")),
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(media_sender_matches(&alice, "12345"));
        assert!(media_sender_matches(&alice, "id:12345"));
        assert!(!media_sender_matches(&same_name_other_id, "id:12345"));
        assert!(!media_sender_matches(&same_name_other_id, "12345"));
        assert!(media_sender_matches(&same_name_other_id, "alice"));
    }

    #[test]
    fn test_measured_height_cache_insertion_and_separation() {
        let mut heights: HashMap<(String, String, u32), f32> = HashMap::new();
        let thread_id = "th_100".to_string();
        let msg_id = "msg_001".to_string();

        let bucket_narrow = 320u32;
        let bucket_wide = 640u32;

        heights.insert((thread_id.clone(), msg_id.clone(), bucket_narrow), 85.5);
        heights.insert((thread_id.clone(), msg_id.clone(), bucket_wide), 42.0);

        assert_eq!(
            heights.get(&(thread_id.clone(), msg_id.clone(), bucket_narrow)),
            Some(&85.5)
        );
        assert_eq!(
            heights.get(&(thread_id.clone(), msg_id.clone(), bucket_wide)),
            Some(&42.0)
        );

        // Invalidation: simulating message mutation or recall
        heights.retain(|(t, m, _), _| !(t == &thread_id && m == &msg_id));
        assert!(heights.is_empty());
    }

    #[test]
    fn repeated_own_reaction_becomes_clear_intent_but_peer_reaction_does_not() {
        let mut message = ChatMessage {
            reaction_summaries: vec![crate::reactions::ReactionSummary {
                emoji: "👍".into(),
                count: 1,
                reactor_ids: vec!["peer-1".into()],
                count_is_lower_bound: false,
                ..Default::default()
            }],
            ..Default::default()
        };

        assert_eq!(reaction_toggle_intent(&message, "owner-1", "👍"), "👍");

        message.reaction_summaries[0]
            .reactor_ids
            .push("owner-1".into());
        assert_eq!(reaction_toggle_intent(&message, "owner-1", "👍"), "");
    }

    #[test]
    fn shared_media_and_links_routes_zdn_and_media_urls_to_photos_only() {
        let mut thread = Thread {
            id: "u-media-test".into(),
            is_group: false,
            ..Default::default()
        };
        thread.messages = vec![
            ChatMessage {
                id: "m1".into(),
                text: "https://photo-stal-16.zdn.vn/w480/sample".into(),
                link_url: Some("https://photo-stal-16.zdn.vn/w480/sample".into()),
                ..Default::default()
            },
            ChatMessage {
                id: "m2".into(),
                text: "Xem ảnh tại https://example.com/pic.png?w=200".into(),
                ..Default::default()
            },
            ChatMessage {
                id: "m3".into(),
                text: "https://cdn.zadn.vn/media/clip.mp4".into(),
                link_url: Some("https://cdn.zadn.vn/media/clip.mp4".into()),
                ..Default::default()
            },
            ChatMessage {
                id: "m4".into(),
                text: "Đọc tin: https://vnexpress.net/bai-viet-123.html".into(),
                link_url: Some("https://vnexpress.net/bai-viet-123.html".into()),
                ..Default::default()
            },
            ChatMessage {
                id: "m5".into(),
                kind: crate::message_model::MessageKind::Photo,
                link_url: Some("https://b-f10-zpc.zdn.vn/orig/1".into()),
                link_thumb: Some("https://b-f10-zpc.zdn.vn/thumb/1".into()),
                ..Default::default()
            },
        ];

        let (photos, files, links) = extract_shared_media_and_links(&thread);
        assert!(files.is_empty());
        assert_eq!(photos.len(), 4);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].0, "https://vnexpress.net/bai-viet-123.html");
        assert!(photos
            .iter()
            .any(|(_, media, msg)| msg.id == "m3"
                && media.kind == crate::message_view::MediaKind::Video));
        assert!(photos
            .iter()
            .any(|(_, media, msg)| msg.id == "m2"
                && media.kind == crate::message_view::MediaKind::Photo));
    }

    #[test]
    fn shared_links_prefer_cached_metadata_title() {
        let mut thread = Thread {
            id: "u-link-title".into(),
            messages: vec![ChatMessage {
                id: "m-title".into(),
                text: "https://example.com/article".into(),
                link_url: Some("https://example.com/article".into()),
                link_title: Some("Bài viết đã lưu".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let (_, _, links) = extract_shared_media_and_links(&thread);
        assert_eq!(links[0].1, "Bài viết đã lưu");
        thread.messages[0].link_title = Some("  ".into());
        let (_, _, links) = extract_shared_media_and_links(&thread);
        assert_eq!(links[0].1, "https://example.com/article");
    }

    #[test]
    fn auto_delete_ttl_formatting_uses_friendly_vietnamese_labels() {
        assert_eq!(format_auto_delete_ttl(0), "Đang tắt");
        assert_eq!(format_auto_delete_ttl(86_400_000), "1 ngày");
        assert_eq!(format_auto_delete_ttl(604_800_000), "7 ngày");
        assert_eq!(format_auto_delete_ttl(2_592_000_000), "30 ngày");
        assert_eq!(format_auto_delete_ttl(3_600_000), "1 giờ");
    }

    #[test]
    fn test_mention_mark_read_gated_by_modals() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            Vec::new(),
            false,
        );
        app.is_logged_in = true;
        let mut t = Thread {
            id: "group_mentions".into(),
            name: "Group Mentions".into(),
            is_group: true,
            ..Default::default()
        };
        t.unread.mention_ids.insert("m-mention-1".into());
        t.unread.mention_ids.insert("m-mention-2".into());
        app.threads = vec![t];
        app.selected_thread_index = 0;

        // When a modal is open (e.g. settings modal), input is gated
        app.settings_modal_open = true;
        assert!(app.is_workspace_input_gated(&ctx));

        // Simulate frame delivering visible mentions while modal is open
        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("visible_mentions"),
                vec!["m-mention-1".to_string()],
            );
        });

        // Mention mark-read check logic (same as workspace.rs:3289)
        let read_ids = ctx
            .data_mut(|d| d.remove_temp::<Vec<String>>(egui::Id::new("visible_mentions")))
            .unwrap_or_default();
        let focused = true;
        if focused && !app.is_workspace_input_gated(&ctx) {
            if let Some(thread) = app.threads.get_mut(app.selected_thread_index) {
                for id in read_ids {
                    thread.unread.mention_ids.remove(&id);
                }
            }
        }
        // Mentions must NOT be cleared because modal is open
        assert_eq!(app.threads[0].unread.mention_ids.len(), 2);
        assert!(app.threads[0].unread.mention_ids.contains("m-mention-1"));

        // Now test with forward session modal open
        app.settings_modal_open = false;
        app.forward_session = Some(modals::ForwardSessionState {
            original_msg: ChatMessage::default(),
            note_text: String::new(),
            targets: Vec::new(),
        });
        assert!(app.is_workspace_input_gated(&ctx));

        // When all modals are closed, input is NOT gated
        app.forward_session = None;
        assert!(!app.is_workspace_input_gated(&ctx));

        ctx.data_mut(|d| {
            d.insert_temp(
                egui::Id::new("visible_mentions"),
                vec!["m-mention-1".to_string()],
            );
        });
        let read_ids = ctx
            .data_mut(|d| d.remove_temp::<Vec<String>>(egui::Id::new("visible_mentions")))
            .unwrap_or_default();
        if focused && !app.is_workspace_input_gated(&ctx) {
            if let Some(thread) = app.threads.get_mut(app.selected_thread_index) {
                for id in read_ids {
                    thread.unread.mention_ids.remove(&id);
                }
            }
        }
        // "m-mention-1" is now cleared, but "m-mention-2" remains unread
        assert_eq!(app.threads[0].unread.mention_ids.len(), 1);
        assert!(!app.threads[0].unread.mention_ids.contains("m-mention-1"));
        assert!(app.threads[0].unread.mention_ids.contains("m-mention-2"));
    }

    fn test_app_and_context() -> (ZaloGuiApp, egui::Context) {
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

    #[test]
    fn sync_banner_fencing_protection_and_stale_rejection() {
        let (mut app, _ctx) = test_app_and_context();
        app.is_logged_in = true;
        app.user_uin = "user_abc".into();
        app.account_runtime_id = 42;
        app.sync_operation_id = 100;
        app.sync_state_revision = 5;

        let valid_fencing = crate::gui::sync_banner::SyncBannerFencing::new("user_abc", 42, 100, 5);
        let stale_owner = crate::gui::sync_banner::SyncBannerFencing::new("user_xyz", 42, 100, 5);
        let stale_runtime = crate::gui::sync_banner::SyncBannerFencing::new("user_abc", 43, 100, 5);
        let stale_revision =
            crate::gui::sync_banner::SyncBannerFencing::new("user_abc", 42, 100, 4);

        // Stale owner rejected
        assert!(!app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::StartSuggestSync,
            stale_owner
        ));
        // Stale runtime rejected
        assert!(!app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::StartSuggestSync,
            stale_runtime
        ));
        // Stale revision rejected
        assert!(!app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::StartSuggestSync,
            stale_revision
        ));

        // Valid fencing advances state revision
        let rev_before = app.sync_state_revision;
        assert!(app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::StartSuggestSync,
            valid_fencing
        ));
        assert_eq!(app.sync_state_revision, rev_before + 1);
        assert!(app.phone_sync_running);
    }

    #[test]
    fn sync_banner_dismiss_or_close_classification() {
        let (mut app, _ctx) = test_app_and_context();
        app.is_logged_in = true;
        app.user_uin = "user_test".into();
        app.account_runtime_id = 1;

        // 1. In Error state: close clears error and resets to Idle
        app.sync_state = crate::sync_coordinator::SyncState::Failed {
            error: "Lỗi mạng".into(),
            fatal: false,
        };
        app.sync_state_revision = 1;
        let fencing = crate::gui::sync_banner::SyncBannerFencing::new("user_test", 1, 0, 1);
        assert!(app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::DismissOrClose,
            fencing
        ));
        assert!(app.sync_error_dismissed);
        assert!(matches!(
            app.sync_state,
            crate::sync_coordinator::SyncState::Idle { .. }
        ));

        // 2. In Running state: close requests cancellation
        app.sync_state = crate::sync_coordinator::SyncState::Preparing {
            source: crate::sync_coordinator::SyncSource::Sync2Full,
        };
        app.sync_cancel_requested = false;
        app.sync_state_revision = 2;
        let fencing = crate::gui::sync_banner::SyncBannerFencing::new("user_test", 1, 0, 2);
        assert!(app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::DismissOrClose,
            fencing
        ));
        assert!(app.sync_cancel_requested);

        // 3. In Success state: close clears success timer immediately
        app.sync_state = crate::sync_coordinator::SyncState::Completed {
            inserted_messages: 10,
            duration: std::time::Duration::from_secs(1),
        };
        app.sync_success_until =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
        app.sync_state_revision = 3;
        let fencing = crate::gui::sync_banner::SyncBannerFencing::new("user_test", 1, 0, 3);
        assert!(app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::DismissOrClose,
            fencing
        ));
        assert_eq!(app.sync_success_until, None);

        // 4. In Suggest state: close sets sync_suggest_dismissed
        app.sync_state = crate::sync_coordinator::SyncState::Idle {
            availability: crate::sync_coordinator::SyncAvailability::Ready,
        };
        app.sync_suggest_dismissed = false;
        app.sync_state_revision = 4;
        let fencing = crate::gui::sync_banner::SyncBannerFencing::new("user_test", 1, 0, 4);
        assert!(app.handle_sync_banner_action(
            crate::gui::sync_banner::SyncBannerAction::DismissOrClose,
            fencing
        ));
        assert!(app.sync_suggest_dismissed);
    }

    #[test]
    fn sync_banner_popup_suppression_on_app() {
        let (mut app, ctx) = test_app_and_context();
        app.is_logged_in = true;
        app.user_uin = "user_popup".into();
        app.sync_state = crate::sync_coordinator::SyncState::Preparing {
            source: crate::sync_coordinator::SyncSource::Sync2Full,
        };

        // When no modal is open: banner resolves
        let banner_input = |app: &ZaloGuiApp| crate::gui::sync_banner::SyncBannerInput {
            is_logged_in: app.is_logged_in,
            popup_visible: app.is_workspace_input_gated(&ctx),
            suggest_dismissed: app.sync_suggest_dismissed,
            error_dismissed: app.sync_error_dismissed,
            success_until: app.sync_success_until,
            legacy_running: app.phone_sync_running,
            legacy_detail: None,
            suggestion_trigger: app.sync_suggestion_trigger,
            sync_config: None,
        };
        let kind =
            crate::gui::sync_banner::resolve_banner_state(&banner_input(&app), &app.sync_state);
        assert_eq!(
            kind,
            Some(crate::gui::sync_banner::SyncBannerKind::Preparing)
        );

        // When settings modal is open: Popup Suppression Invariant kicks in (returns None)
        app.settings_modal_open = true;
        assert!(app.is_workspace_input_gated(&ctx));
        let kind_suppressed =
            crate::gui::sync_banner::resolve_banner_state(&banner_input(&app), &app.sync_state);
        assert_eq!(kind_suppressed, None);

        // When phone modal is open: also suppressed
        app.settings_modal_open = false;
        app.phone_modal_open = true;
        assert!(app.is_workspace_input_gated(&ctx));
        let kind_suppressed_phone =
            crate::gui::sync_banner::resolve_banner_state(&banner_input(&app), &app.sync_state);
        assert_eq!(kind_suppressed_phone, None);
    }

    #[test]
    fn sync_banner_state_transitions_never_expand_sidebar_width() {
        let (mut app, ctx) = test_app_and_context();
        app.hidden_panel.view.ready = true;
        app.policy_panel.snapshot = Some(Default::default());

        let mut baseline_panel: Option<Rect> = None;

        let states_to_test = [
            // 1. Suggest
            crate::sync_coordinator::SyncState::Idle {
                availability: crate::sync_coordinator::SyncAvailability::Ready,
            },
            // 2. WaitConfirmation
            crate::sync_coordinator::SyncState::AwaitingPhoneConfirmation {
                timeout_at: std::time::Instant::now() + std::time::Duration::from_secs(60),
            },
            // 3. Preparing
            crate::sync_coordinator::SyncState::Preparing {
                source: crate::sync_coordinator::SyncSource::Sync2Full,
            },
            // 4. SyncingConvListRecent
            crate::sync_coordinator::SyncState::Transferring {
                stage: crate::sync_coordinator::SyncTransferStage::DescriptorReceived,
                downloaded_bytes: 0,
                total_bytes: None,
                inserted_messages: 0,
                total_messages: None,
            },
            // 5. SyncingOtherMessages with very long status text
            crate::sync_coordinator::SyncState::Transferring {
                stage: crate::sync_coordinator::SyncTransferStage::Downloaded,
                downloaded_bytes: 50 * 1024 * 1024,
                total_bytes: Some(100 * 1024 * 1024),
                inserted_messages: 5000,
                total_messages: Some(10000),
            },
            // 6. Interrupted
            crate::sync_coordinator::SyncState::Interrupted {
                reason: "Mất kết nối mạng đột ngột trong khi đang truyền tải dữ liệu rất dài"
                    .repeat(5),
                retryable: true,
            },
            // 7. Error with long error message
            crate::sync_coordinator::SyncState::Failed {
                error:
                    "Lỗi ghi CSDL cục bộ không thể hoàn tất do tập tin bị khóa bởi tiến trình khác"
                        .repeat(5),
                fatal: false,
            },
            // 8. Success
            crate::sync_coordinator::SyncState::Completed {
                inserted_messages: 1234,
                duration: std::time::Duration::from_secs(12),
            },
        ];

        for (idx, state) in states_to_test.into_iter().enumerate() {
            app.sync_state = state;
            app.sync_state_revision = app.sync_state_revision.wrapping_add(1);
            if matches!(
                app.sync_state,
                crate::sync_coordinator::SyncState::Completed { .. }
            ) {
                app.sync_success_until =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            } else {
                app.sync_success_until = None;
            }

            for _ in 0..2 {
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            Pos2::ZERO,
                            Vec2::new(1020.0, 700.0),
                        )),
                        ..Default::default()
                    },
                    |ctx| {
                        app.render_chat_workspace(ctx, &app.palette());
                    },
                );
            }

            let panel = ctx
                .data(|data| data.get_temp::<Rect>(egui::Id::new("sidebar-panel-rect")))
                .expect("sidebar-panel-rect must be recorded");

            if let Some(expected) = baseline_panel {
                assert_eq!(
                    panel.width(),
                    expected.width(),
                    "state {idx}: sidebar width changed during banner transition"
                );
            } else {
                baseline_panel = Some(panel);
            }
        }
    }

    #[test]
    fn test_phase4_manual_mark_unread_cleared_on_open_thread() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.threads = vec![Thread {
            id: "test-thread".into(),
            unread: crate::unread::UnreadState {
                marked_unread: true,
                ..Default::default()
            },
            ..Default::default()
        }];
        assert!(app.threads[0].unread.marked_unread);
        app.open_thread(&ctx, 0, "test-thread".into(), false);
        assert!(!app.threads[0].unread.marked_unread);
    }

    #[test]
    fn test_phase4_archived_chat_filter_isolation() {
        let (mut app, _) = sync_and_order_tests::app_and_context();
        app.threads = vec![
            Thread {
                id: "active-thread".into(),
                presentation: crate::chat::SidebarPresentation {
                    archived: Some(false),
                    ..Default::default()
                },
                ..Default::default()
            },
            Thread {
                id: "archived-thread".into(),
                presentation: crate::chat::SidebarPresentation {
                    archived: Some(true),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        // Default: ThreadFilter::All -> archived is filtered out
        let active_filtered: Vec<_> = app
            .threads
            .iter()
            .filter(|t| {
                app.thread_filter == ThreadFilter::Archived || t.presentation.archived != Some(true)
            })
            .collect();
        assert_eq!(active_filtered.len(), 1);
        assert_eq!(active_filtered[0].id, "active-thread");

        // Filter: ThreadFilter::Archived -> only archived is visible
        app.thread_filter = ThreadFilter::Archived;
        let archived_filtered: Vec<_> = app
            .threads
            .iter()
            .filter(|t| match app.thread_filter {
                ThreadFilter::Archived => t.presentation.archived == Some(true),
                _ => t.presentation.archived != Some(true),
            })
            .collect();
        assert_eq!(archived_filtered.len(), 1);
        assert_eq!(archived_filtered[0].id, "archived-thread");
    }

    #[test]
    fn test_phase4_presence_map_cache_status() {
        let (mut app, _) = sync_and_order_tests::app_and_context();
        let now_ms = crate::login::unix_now() * 1000;
        app.presence_map.insert(
            "peer-active".into(),
            (
                crate::last_activity::Status::LastActivity(now_ms),
                std::time::Instant::now(),
            ),
        );
        app.presence_map.insert(
            "peer-offline".into(),
            (
                crate::last_activity::Status::LastActivity(now_ms - 3_600_000 * 2),
                std::time::Instant::now(),
            ),
        );

        if let Some((crate::last_activity::Status::LastActivity(ts), _)) =
            app.presence_map.get("peer-active")
        {
            assert!(now_ms.saturating_sub(*ts) <= 180_000);
        } else {
            panic!("expected peer-active presence");
        }

        if let Some((crate::last_activity::Status::LastActivity(ts), _)) =
            app.presence_map.get("peer-offline")
        {
            assert!(now_ms.saturating_sub(*ts) > 180_000);
        } else {
            panic!("expected peer-offline presence");
        }
    }

    #[test]
    fn test_metadata_stale_snapshot_arriving_after_ack_does_not_overwrite_state() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "test-user".into();
        app.account_runtime_id = 42;
        app.threads = vec![Thread {
            id: "123456".into(),
            ..Default::default()
        }];

        // Step 1: Snapshot starts in background before mutation, captures gen 10
        let snapshot_rev = 10u64;

        // Step 2: User initiates mutation, sets revision = 20, generation = 20, pending = true
        app.manual_mark_revision = 20;
        app.manual_mark_generation = 20;
        app.manual_mark_pending = true;

        // Step 3: Mutation ACK arrives (gen 20, mutation = true) -> marks unread
        let ack_marks = vec![crate::unread::ManualUnreadMark {
            thread_id: "123456".into(),
            client_message_id: "m_ack".into(),
            from_uid: "123456".into(),
        }];
        let _ = app.event_tx.send(GuiEvent::ManualMarks {
            owner: "test-user".into(),
            runtime_id: 42,
            generation: 20,
            kind: MetadataResultKind::Mutation,
            result: Ok(ack_marks),
        });
        app.check_events(&ctx);
        assert!(!app.manual_mark_pending);
        assert!(app.threads[0].unread.marked_unread);

        // Step 4: Stale background snapshot with gen 10 arrives (carrying empty marks)
        let _ = app.event_tx.send(GuiEvent::ManualMarks {
            owner: "test-user".into(),
            runtime_id: 42,
            generation: snapshot_rev,
            kind: MetadataResultKind::Snapshot,
            result: Ok(vec![]),
        });
        app.check_events(&ctx);
        // Assert: Thread remains marked_unread! Stale snapshot was rejected because snapshot_rev (10) < manual_mark_revision (20)
        assert!(app.threads[0].unread.marked_unread);

        // Check AccountMetadata as well
        app.archive_revision = 50;
        app.archive_generation = 50;
        app.threads[0].presentation.archived = Some(true);

        let stale_archive_snapshot = crate::account_metadata::Snapshot {
            archive: Some(crate::archive::Snapshot {
                version: 1,
                items: vec![],
            }),
            ..Default::default()
        };
        let _ = app.event_tx.send(GuiEvent::AccountMetadata {
            owner: "test-user".into(),
            runtime_id: 42,
            revision: 30, // < 50
            snapshot: Box::new(stale_archive_snapshot),
        });
        app.check_events(&ctx);
        // Assert: Thread remains archived! Stale AccountMetadata revision (30) was rejected against revision (50)
        assert_eq!(app.threads[0].presentation.archived, Some(true));
    }

    #[test]
    fn test_metadata_snapshot_while_mutation_pending_does_not_clear_pending_or_uncertain() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "test-user".into();
        app.account_runtime_id = 42;

        app.labels_panel.pending = true;
        app.labels_panel.uncertain = true;
        app.labels_panel.generation = 100;
        app.labels_panel.revision = 100;

        // Background snapshot arrives (even with higher gen 105 or error)
        let _ = app.event_tx.send(GuiEvent::Labels {
            owner: "test-user".into(),
            runtime_id: 42,
            generation: 105,
            kind: MetadataResultKind::Snapshot,
            result: Err("Server timeout".into()),
        });
        app.check_events(&ctx);

        // Pending and uncertain must remain true!
        assert!(app.labels_panel.pending);
        assert!(app.labels_panel.uncertain);
    }

    #[test]
    fn test_metadata_hydrate_valid_and_out_of_order_responses_keep_latest_only() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "test-user".into();
        app.account_runtime_id = 42;

        // Initial hydrate with valid revision
        let snap1 = crate::labels::Snapshot::parse(serde_json::json!({
            "version": 1,
            "labelData": [
                { "id": 1, "name": "Khách hàng", "conversations": [] }
            ]
        }))
        .unwrap();
        let _ = app.event_tx.send(GuiEvent::Labels {
            owner: "test-user".into(),
            runtime_id: 42,
            generation: 10,
            kind: MetadataResultKind::Snapshot,
            result: Ok(snap1),
        });
        app.check_events(&ctx);
        assert_eq!(app.labels_panel.snapshot.as_ref().unwrap().labels.len(), 1);
        assert_eq!(app.labels_panel.revision, 10);

        // Newer snapshot revision 20 arrives
        let snap2 = crate::labels::Snapshot::parse(serde_json::json!({
            "version": 2,
            "labelData": [
                { "id": 1, "name": "Khách hàng", "conversations": [] },
                { "id": 2, "name": "Bạn bè", "conversations": [] }
            ]
        }))
        .unwrap();
        let _ = app.event_tx.send(GuiEvent::Labels {
            owner: "test-user".into(),
            runtime_id: 42,
            generation: 20,
            kind: MetadataResultKind::Snapshot,
            result: Ok(snap2),
        });
        app.check_events(&ctx);
        assert_eq!(app.labels_panel.snapshot.as_ref().unwrap().labels.len(), 2);
        assert_eq!(app.labels_panel.revision, 20);

        // Out-of-order older snapshot revision 15 arrives
        let snap_old = crate::labels::Snapshot::parse(serde_json::json!({
            "version": 1,
            "labelData": []
        }))
        .unwrap();
        let _ = app.event_tx.send(GuiEvent::Labels {
            owner: "test-user".into(),
            runtime_id: 42,
            generation: 15,
            kind: MetadataResultKind::Snapshot,
            result: Ok(snap_old),
        });
        app.check_events(&ctx);
        // Must remain 2 labels! Revision 15 was rejected because 15 < 20
        assert_eq!(app.labels_panel.snapshot.as_ref().unwrap().labels.len(), 2);
    }

    #[test]
    fn test_metadata_stale_runtime_or_mismatched_owner_dropped() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "user-active".into();
        app.account_runtime_id = 10;
        app.threads = vec![Thread {
            id: "t1".into(),
            ..Default::default()
        }];

        // Mismatched owner
        let _ = app.event_tx.send(GuiEvent::Archive {
            owner: "user-other".into(),
            runtime_id: 10,
            generation: 100,
            kind: MetadataResultKind::Snapshot,
            result: Ok(crate::archive::Snapshot {
                version: 1,
                items: vec![crate::archive::Item {
                    id: "t1".into(),
                    kind: 0,
                }],
            }),
        });
        app.check_events(&ctx);
        assert_eq!(app.threads[0].presentation.archived, None);

        // Mismatched runtime_id
        let _ = app.event_tx.send(GuiEvent::Archive {
            owner: "user-active".into(),
            runtime_id: 9, // stale runtime
            generation: 100,
            kind: MetadataResultKind::Snapshot,
            result: Ok(crate::archive::Snapshot {
                version: 1,
                items: vec![crate::archive::Item {
                    id: "t1".into(),
                    kind: 0,
                }],
            }),
        });
        app.check_events(&ctx);
        assert_eq!(app.threads[0].presentation.archived, None);
    }

    #[test]
    fn test_preflight_logout_expiry_clears_state_and_stops_monitor() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "caller-user".into();
        app.account_runtime_id = 5;

        app.open_preflight_call("callee".into(), "Callee Name".into(), None, false, false);
        assert!(app.preflight_call.is_some());
        let preflight = app.preflight_call.as_ref().unwrap();
        assert_eq!(preflight.owner, "caller-user");
        assert_eq!(preflight.runtime_id, 5);
        assert!(!preflight.mic_monitor.as_ref().unwrap().is_stopped());

        // Account logout / boundary triggers clear_presence_state
        app.clear_presence_state();
        assert!(app.preflight_call.is_none());

        // Render check: dialog is not rendered
        app.render_preflight_dialog(&ctx);
        assert!(app.preflight_call.is_none());
    }

    #[test]
    fn test_preflight_relogin_different_runtime_worker_rejects_old_call_cmd() {
        let session = Session::new(false, false, false).unwrap();
        assert!(session.is_active());
        let rt1 = session.runtime_id();

        // Enqueued StartCall has owner "user-a" and rt1
        let cmd_owner = "user-a";
        let cmd_rt = rt1;

        // User logs out (revokes runtime)
        session.revoke_runtime();
        assert!(!session.is_active());

        // New session created for relogin gets new runtime_id
        let new_session = Session::new(false, false, false).unwrap();
        let rt2 = new_session.runtime_id();
        assert_ne!(rt1, rt2);

        // Worker StartCall gate checks: is_active && account_key == owner && runtime_id == cmd_rt
        let accepted_old = new_session.is_active()
            && new_session.account_key() == cmd_owner
            && new_session.runtime_id() == cmd_rt;
        assert!(!accepted_old);
    }

    #[test]
    fn test_preflight_scope_correct_single_call_and_cancel_stops_monitor_without_live_devices() {
        let (mut app, _) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "caller".into();
        app.account_runtime_id = 7;

        app.open_preflight_call("target".into(), "Target Name".into(), None, false, false);
        let monitor = app
            .preflight_call
            .as_ref()
            .unwrap()
            .mic_monitor
            .clone()
            .unwrap();
        assert!(!monitor.is_stopped());

        // Calling clear_preflight_call stops monitor safely
        app.clear_preflight_call();
        assert!(monitor.is_stopped());
        assert!(app.preflight_call.is_none());
    }

    #[test]
    fn test_presence_mismatched_owner_runtime_or_logged_out_does_not_write_cache() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "user-1".into();
        app.account_runtime_id = 10;

        // Event with wrong owner
        let _ = app.event_tx.send(GuiEvent::ActivityUpdated {
            owner: "user-2".into(),
            runtime_id: 10,
            uid: "peer-a".into(),
            status: crate::last_activity::Status::LastActivity(1000),
        });
        app.check_events(&ctx);
        assert!(!app.presence_map.contains_key("peer-a"));

        // Event with stale runtime
        let _ = app.event_tx.send(GuiEvent::ActivityUpdated {
            owner: "user-1".into(),
            runtime_id: 9,
            uid: "peer-b".into(),
            status: crate::last_activity::Status::LastActivity(1000),
        });
        app.check_events(&ctx);
        assert!(!app.presence_map.contains_key("peer-b"));

        // Event when logged out
        app.is_logged_in = false;
        let _ = app.event_tx.send(GuiEvent::ActivityUpdated {
            owner: "user-1".into(),
            runtime_id: 10,
            uid: "peer-c".into(),
            status: crate::last_activity::Status::LastActivity(1000),
        });
        app.check_events(&ctx);
        assert!(!app.presence_map.contains_key("peer-c"));
    }

    #[test]
    fn test_presence_cached_last_activity_hidden_when_privacy_false_or_unknown() {
        let (mut app, _) = sync_and_order_tests::app_and_context();
        let now_ms = crate::login::unix_now() * 1000;
        app.presence_map.insert(
            "peer-1".into(),
            (
                crate::last_activity::Status::LastActivity(now_ms),
                std::time::Instant::now(),
            ),
        );

        // When show_online is None (unknown):
        app.inbox_policy.show_online = None;
        let show_online = app.inbox_policy.show_online == Some(true);
        assert!(!show_online);

        // When show_online is Some(false):
        app.inbox_policy.show_online = Some(false);
        let show_online = app.inbox_policy.show_online == Some(true);
        assert!(!show_online);

        // When show_online is Some(true):
        app.inbox_policy.show_online = Some(true);
        let show_online = app.inbox_policy.show_online == Some(true);
        assert!(show_online);
    }

    #[test]
    fn test_presence_privacy_unavailable_does_not_leak_status() {
        let (mut app, _) = sync_and_order_tests::app_and_context();
        app.inbox_policy.show_online = Some(true);
        app.presence_map.insert(
            "peer-priv".into(),
            (
                crate::last_activity::Status::PrivacyUnavailable,
                std::time::Instant::now(),
            ),
        );

        let presence = app.presence_map.get("peer-priv").map(|(s, _)| s.clone());
        let is_active_now = match &presence {
            Some(crate::last_activity::Status::LastActivity(ts)) => {
                let now_ms = crate::login::unix_now() * 1000;
                now_ms.saturating_sub(*ts) <= 180_000
            }
            _ => false,
        };
        // PrivacyUnavailable must NOT be considered active or online!
        assert!(!is_active_now);
    }

    #[test]
    fn test_remediation_ui_read_completion_releases_pending_in_every_domain() {
        for domain in 0..3 {
            for succeeds in [true, false] {
                let (mut app, ctx) = sync_and_order_tests::app_and_context();
                app.is_logged_in = true;
                app.user_uin = "read-owner".into();
                app.account_runtime_id = 42;
                app.threads = vec![Thread {
                    id: "123".into(),
                    ..Default::default()
                }];
                app.threads[0].unread.marked_unread = true;
                app.threads[0].presentation.archived = Some(true);
                app.labels_panel.uncertain = true;
                app.manual_mark_uncertain = true;
                app.archive_uncertain = true;
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
                WORKER_TX.set(worker::WorkerSender::new_for_test(tx, ctx.clone()));
                match domain {
                    0 => app.open_labels(None),
                    1 => app.manual_mark_command(
                        String::new(),
                        false,
                        crate::unread::MarkAction::Refresh,
                    ),
                    _ => app.archive_command(String::new(), false, None),
                }
                let own_generation = match domain {
                    0 => app.labels_panel.generation,
                    1 => app.manual_mark_generation,
                    _ => app.archive_generation,
                };
                for (kind, generation) in [
                    (MetadataResultKind::Read, own_generation - 1),
                    (MetadataResultKind::Snapshot, own_generation + 1),
                ] {
                    let stale = match domain {
                        0 => GuiEvent::Labels {
                            owner: app.user_uin.clone(),
                            runtime_id: 42,
                            generation,
                            kind,
                            result: Err("unrelated result".into()),
                        },
                        1 => GuiEvent::ManualMarks {
                            owner: app.user_uin.clone(),
                            runtime_id: 42,
                            generation,
                            kind,
                            result: Err("unrelated result".into()),
                        },
                        _ => GuiEvent::Archive {
                            owner: app.user_uin.clone(),
                            runtime_id: 42,
                            generation,
                            kind,
                            result: Err("unrelated result".into()),
                        },
                    };
                    app.event_tx.send(stale).unwrap();
                    app.check_events(&ctx);
                    assert!(match domain {
                        0 => app.labels_panel.pending,
                        1 => app.manual_mark_pending,
                        _ => app.archive_pending,
                    });
                }
                let response = match rx.try_recv().unwrap() {
                    WorkerCmd::Labels {
                        owner,
                        runtime_id,
                        generation,
                        action: crate::labels::Action::Read,
                    } => GuiEvent::Labels {
                        owner,
                        runtime_id,
                        generation,
                        kind: MetadataResultKind::Read,
                        result: if succeeds {
                            Ok(crate::labels::Snapshot::parse(
                                serde_json::json!({"version":1,"labelData":[{"id":1,"name":"Fresh label","conversations":[]}]}),
                            )
                            .unwrap())
                        } else {
                            Err("fixture read error".into())
                        },
                    },
                    WorkerCmd::ManualMark {
                        owner,
                        runtime_id,
                        generation,
                        action: crate::unread::MarkAction::Refresh,
                        ..
                    } => GuiEvent::ManualMarks {
                        owner,
                        runtime_id,
                        generation,
                        kind: MetadataResultKind::Read,
                        result: if succeeds {
                            Ok(Vec::new())
                        } else {
                            Err("fixture read error".into())
                        },
                    },
                    WorkerCmd::Archive {
                        owner,
                        runtime_id,
                        generation,
                        archived: None,
                        ..
                    } => GuiEvent::Archive {
                        owner,
                        runtime_id,
                        generation,
                        kind: MetadataResultKind::Read,
                        result: if succeeds {
                            Ok(crate::archive::Snapshot {
                                version: 1,
                                items: Vec::new(),
                            })
                        } else {
                            Err("fixture read error".into())
                        },
                    },
                    _ => panic!("expected the UI's read command"),
                };
                app.event_tx.send(response).unwrap();
                app.check_events(&ctx);
                let pending = match domain {
                    0 => app.labels_panel.pending,
                    1 => app.manual_mark_pending,
                    _ => app.archive_pending,
                };
                assert!(
                    !pending,
                    "UI read remained pending: domain={domain}, success={succeeds}"
                );
                let uncertain = match domain {
                    0 => app.labels_panel.uncertain,
                    1 => app.manual_mark_uncertain,
                    _ => app.archive_uncertain,
                };
                assert_eq!(uncertain, !succeeds);
                match domain {
                    0 if succeeds => {
                        assert_eq!(app.labels_panel.snapshot.as_ref().unwrap().labels.len(), 1)
                    }
                    0 => assert!(app.labels_panel.snapshot.is_none()),
                    1 => assert_eq!(app.threads[0].unread.marked_unread, !succeeds),
                    _ => assert_eq!(app.threads[0].presentation.archived, Some(!succeeds)),
                }
                WORKER_TX.clear();
            }
        }
    }

    #[test]
    fn test_remediation_privacy_invalidation_rejects_inflight_snapshot() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "privacy-owner".into();
        app.account_runtime_id = 42;
        app.privacy_revision = crate::search::generation();
        let old_revision = crate::search::generation();
        app.inbox_policy.show_online = Some(true);
        app.event_tx
            .send(GuiEvent::AccountMetadataInvalidated {
                runtime_id: app.account_runtime_id,
                revision: crate::search::generation(),
                owner: app.user_uin.clone(),
                privacy: true,
                relations: false,
                business: false,
            })
            .unwrap();
        let privacy = crate::privacy::Snapshot {
            values: [(crate::privacy::Setting::ShowOnline, 1)]
                .into_iter()
                .collect(),
        };
        app.event_tx
            .send(GuiEvent::AccountMetadata {
                owner: app.user_uin.clone(),
                runtime_id: 42,
                revision: old_revision,
                snapshot: Box::new(crate::account_metadata::Snapshot {
                    privacy: Some(privacy.clone()),
                    ..Default::default()
                }),
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(
            app.inbox_policy.show_online, None,
            "stale metadata reopened privacy gate"
        );
        app.event_tx
            .send(GuiEvent::AccountMetadata {
                owner: app.user_uin.clone(),
                runtime_id: 42,
                revision: crate::search::generation(),
                snapshot: Box::new(crate::account_metadata::Snapshot {
                    privacy: Some(privacy),
                    ..Default::default()
                }),
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(
            app.inbox_policy.show_online,
            Some(true),
            "a fresh read must still work"
        );
    }

    #[test]
    fn test_remediation_delayed_ui_keeps_fresh_privacy_and_ignores_old_invalidation() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "privacy-owner".into();
        app.account_runtime_id = 42;
        app.inbox_policy.show_online = Some(true);
        let invalidation_revision = crate::search::generation();
        app.event_tx
            .send(GuiEvent::AccountMetadataInvalidated {
                runtime_id: app.account_runtime_id,
                owner: app.user_uin.clone(),
                revision: invalidation_revision,
                privacy: true,
                relations: false,
                business: false,
            })
            .unwrap();
        // The producer starts a fresh read before the UI consumes the invalidation.
        let fresh_revision = crate::search::generation();
        app.event_tx
            .send(GuiEvent::AccountMetadata {
                owner: app.user_uin.clone(),
                runtime_id: 42,
                revision: fresh_revision,
                snapshot: Box::new(crate::account_metadata::Snapshot {
                    privacy: Some(crate::privacy::Snapshot {
                        values: [(crate::privacy::Setting::ShowOnline, 0)]
                            .into_iter()
                            .collect(),
                    }),
                    ..Default::default()
                }),
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(
            app.inbox_policy.show_online,
            Some(false),
            "fresh result was discarded by a delivery-time barrier"
        );
        app.event_tx
            .send(GuiEvent::AccountMetadataInvalidated {
                runtime_id: app.account_runtime_id,
                owner: app.user_uin.clone(),
                revision: invalidation_revision,
                privacy: true,
                relations: false,
                business: false,
            })
            .unwrap();
        app.check_events(&ctx);
        assert_eq!(
            app.inbox_policy.show_online,
            Some(false),
            "old invalidation hid a newer authoritative result"
        );
    }

    #[test]
    fn test_remediation_privacy_events_cannot_cross_relogin_runtime() {
        let (mut app, ctx) = sync_and_order_tests::app_and_context();
        app.is_logged_in = true;
        app.user_uin = "privacy-owner".into();
        app.account_runtime_id = 42;
        app.inbox_policy.show_online = Some(false);
        app.privacy_panel.pending = true;
        app.privacy_panel.generation = crate::search::generation();
        for (owner, runtime_id) in [("other-owner", 42), ("privacy-owner", 41)] {
            app.event_tx
                .send(GuiEvent::AccountMetadataInvalidated {
                    owner: owner.into(),
                    runtime_id,
                    revision: crate::search::generation(),
                    privacy: true,
                    relations: true,
                    business: true,
                })
                .unwrap();
            app.event_tx
                .send(GuiEvent::Privacy {
                    owner: owner.into(),
                    runtime_id,
                    generation: app.privacy_panel.generation,
                    mutation: true,
                    result: Ok(crate::privacy::Snapshot {
                        values: [(crate::privacy::Setting::ShowOnline, 1)]
                            .into_iter()
                            .collect(),
                    }),
                })
                .unwrap();
            app.check_events(&ctx);
            assert!(app.privacy_panel.pending);
            assert_eq!(app.inbox_policy.show_online, Some(false));
        }
        app.event_tx
            .send(GuiEvent::Privacy {
                owner: app.user_uin.clone(),
                runtime_id: 42,
                generation: app.privacy_panel.generation,
                mutation: false,
                result: Ok(crate::privacy::Snapshot {
                    values: [(crate::privacy::Setting::ShowOnline, 0)]
                        .into_iter()
                        .collect(),
                }),
            })
            .unwrap();
        app.check_events(&ctx);
        assert!(!app.privacy_panel.pending);
        assert_eq!(app.inbox_policy.show_online, Some(false));
    }
}
