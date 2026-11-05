//! Scoped in-memory attachment drafts and hit-tested desktop file drops.
use super::*;
use std::path::PathBuf;

const PER_THREAD: usize = 16;
const TOTAL: usize = 64;

pub struct Prepared {
    pub stamp: crate::outgoing::FileStamp,
    pub size: u64,
    pub image: Option<image::RgbaImage>,
}

pub(super) struct Item {
    pub id: u64,
    pub path: PathBuf,
    pub result: Option<Result<crate::outgoing::FileStamp, String>>,
    pub size: u64,
    pub texture: Option<TextureHandle>,
}

#[derive(Default)]
pub(super) struct Drafts {
    owner: String,
    threads: HashMap<String, Vec<Item>>,
    deferred: Option<DropCapture>,
}

struct DropCapture {
    thread: String,
    paths: Vec<PathBuf>,
    position: Option<Pos2>,
    blocked: bool,
}

impl Drafts {
    pub fn sync_owner(&mut self, owner: &str) {
        if self.owner != owner {
            self.threads.clear();
            self.deferred = None;
            self.owner = owner.into();
        }
    }
    /// RawInput moves dropped files out on the first pass. Preserve only a discarded
    /// pass's event, never replay it into a later frame, account or conversation.
    fn take_drop(
        &mut self,
        ctx: &egui::Context,
        thread: &str,
        position: Option<Pos2>,
        blocked: bool,
    ) -> Result<Option<DropCapture>, &'static str> {
        let previous = self.deferred.take();
        let files = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        let capture = if !files.is_empty() {
            if files.len() > PER_THREAD {
                return Err("Mỗi lần chỉ nhận tối đa 16 tệp; chưa gửi hay thêm tệp nào");
            }
            let paths: Vec<_> = files.into_iter().filter_map(|f| f.path).collect();
            if paths.is_empty() {
                return Err("Không đọc được đường dẫn tệp kéo thả");
            }
            Some(DropCapture {
                thread: thread.into(),
                paths,
                position,
                blocked,
            })
        } else if ctx.output(|o| o.num_completed_passes) > 0 {
            previous.filter(|c| c.thread == thread)
        } else {
            None
        };
        if ctx.will_discard() {
            self.deferred = capture;
            Ok(None)
        } else {
            Ok(capture)
        }
    }
    pub fn has_deferred(&self) -> bool {
        self.deferred.is_some()
    }
    pub fn len(&self, thread: &str) -> usize {
        self.threads.get(thread).map_or(0, Vec::len)
    }
    fn insert(&mut self, thread: &str, path: PathBuf) -> Option<u64> {
        if self.owner.is_empty()
            || self.len(thread) >= PER_THREAD
            || self.threads.values().map(Vec::len).sum::<usize>() >= TOTAL
        {
            return None;
        }
        let items = self.threads.entry(thread.into()).or_default();
        if items.iter().any(|i| i.path == path) {
            return None;
        }
        let id = crate::chat::next_client_message_id();
        items.push(Item {
            id,
            path,
            result: None,
            size: 0,
            texture: None,
        });
        Some(id)
    }
    pub fn remove_thread(&mut self, owner: &str, thread: &str) {
        if self.owner == owner {
            self.threads.remove(thread);
            if self.deferred.as_ref().is_some_and(|c| c.thread == thread) {
                self.deferred = None;
            }
        }
    }
    pub fn complete(
        &mut self,
        ctx: &egui::Context,
        owner: &str,
        thread: &str,
        id: u64,
        result: Result<Prepared, String>,
    ) {
        if self.owner != owner {
            return;
        }
        let Some(item) = self
            .threads
            .get_mut(thread)
            .and_then(|items| items.iter_mut().find(|i| i.id == id))
        else {
            return;
        };
        match result {
            Ok(prepared) => {
                item.size = prepared.size;
                item.texture = prepared.image.map(|image| {
                    ctx.load_texture(
                        format!("drop_preview_{id}"),
                        egui::ColorImage::from_rgba_unmultiplied(
                            [image.width() as usize, image.height() as usize],
                            image.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    )
                });
                item.result = Some(Ok(prepared.stamp));
            }
            Err(error) => item.result = Some(Err(error)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Target {
    Send,
    Compose,
}

fn target(
    position: Option<Pos2>,
    chat: egui::Rect,
    compose: egui::Rect,
    blocked: bool,
) -> Option<Target> {
    if blocked {
        return None;
    }
    let position = position?;
    if compose.contains(position) {
        Some(Target::Compose)
    } else if chat.contains(position) {
        Some(Target::Send)
    } else {
        None
    }
}

/// Worker-only thumbnail preparation; oversized/unsupported images retain a generic file card.
pub(super) async fn prepare(path: PathBuf) -> anyhow::Result<Prepared> {
    let stamp = crate::outgoing::FileStamp::read(&path).await?;
    let size = tokio::fs::metadata(&path).await?.len();
    let read_path = path.clone();
    let image = tokio::task::spawn_blocking(move || {
        let bytes = crate::limits::read_file(&read_path, crate::limits::IMAGE_BYTES).ok()?;
        Some(
            crate::limits::decode_image(&bytes)
                .ok()?
                .thumbnail(64, 64)
                .to_rgba8(),
        )
    })
    .await?;
    anyhow::ensure!(
        stamp == crate::outgoing::FileStamp::read(&path).await?,
        "Tệp đã thay đổi khi tạo bản xem trước"
    );
    Ok(Prepared { stamp, size, image })
}

pub struct Batch {
    pub thread: String,
    pub group: bool,
    pub caption: String,
    pub original: bool,
    pub files: Vec<(PathBuf, Option<crate::outgoing::FileStamp>)>,
}

impl ZaloGuiApp {
    fn file_drop_position(&self, ctx: &egui::Context) -> Option<Pos2> {
        // winit 0.30 discards OLE DragOver coordinates on Windows, so the egui pointer
        // can remain at its pre-drag position. Sample the live client cursor instead.
        #[cfg(windows)]
        {
            self.native_hwnd
                .and_then(|hwnd| crate::drop_cursor_windows::position(hwnd))
                .map(|(x, y)| {
                    let scale = ctx.pixels_per_point();
                    egui::pos2(x as f32 / scale, y as f32 / scale)
                })
        }
        #[cfg(not(windows))]
        {
            ctx.input(|i| i.pointer.hover_pos())
        }
    }
    pub(super) fn file_drop_blocked(&self, ctx: &egui::Context) -> bool {
        !self.is_logged_in
            || self.settings_modal_open
            || self.phone_modal_open
            || self.image_viewer.is_some()
            || self.forward_msg.is_some()
            || self.quick_schedule.is_some()
            || self.tools.open
            || self.active_call.is_some()
            || self.screen_capture_pending
            || self.profile_menu_open
            || self.sticker_picker_open
            || self.contacts_view_open
            || ctx.memory(|m| m.any_popup_open())
    }

    pub(super) fn render_attachment_draft(
        &mut self,
        ui: &mut egui::Ui,
        thread: &str,
        pal: &ThemePalette,
        budget: f32,
    ) -> egui::Rect {
        if self.attachment_drafts.len(thread) == 0 {
            return egui::Rect::NOTHING;
        }
        let response = ui.scope(|ui| {
            let Some(items) = self.attachment_drafts.threads.get_mut(thread) else {
                return;
            };
            if items.is_empty() {
                return;
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} tệp chờ gửi", items.len())).color(pal.text_secondary),
                )
                .on_hover_text(crate::i18n::tr(
                    "Nhập chú thích bên dưới. Tệp chờ chỉ được giữ trong phiên làm việc này.",
                ));
                if ui
                    .button("×")
                    .on_hover_text(crate::i18n::tr("Bỏ tất cả tệp chờ"))
                    .clicked()
                {
                    items.clear();
                }
            });
            let mut remove = None;
            egui::ScrollArea::vertical()
                .id_salt(("attachment_draft", thread))
                .min_scrolled_height(0.0)
                .max_height((budget - 28.0).clamp(0.0, 112.0))
                .show(ui, |ui| {
                    for (index, item) in items.iter().enumerate() {
                        ui.horizontal(|ui| {
                            if let Some(tex) = &item.texture {
                                ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::splat(40.0)));
                            } else {
                                ui.label(
                                    RichText::new(lucide::PAPERCLIP).color(pal.text_secondary),
                                );
                            }
                            ui.vertical(|ui| {
                                ui.set_max_width((ui.available_width() - 28.0).max(16.0));
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(
                                            item.path
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy(),
                                        )
                                        .color(pal.text_primary),
                                    )
                                    .truncate(),
                                );
                                let status = match &item.result {
                                    None => "Đang đọc tệp…".into(),
                                    Some(Ok(_)) => crate::chat::format_file_size(item.size),
                                    Some(Err(e)) => e.clone(),
                                };
                                ui.label(RichText::new(status).small().color(
                                    if matches!(item.result, Some(Err(_))) {
                                        pal.danger
                                    } else {
                                        pal.text_muted
                                    },
                                ));
                            });
                            if ui
                                .button("×")
                                .on_hover_text(crate::i18n::tr("Bỏ tệp khỏi danh sách chờ"))
                                .clicked()
                            {
                                remove = Some(index);
                            }
                        });
                    }
                });
            if let Some(index) = remove {
                items.remove(index);
            }
        });
        if self.attachment_drafts.len(thread) == 0 {
            self.attachment_drafts.threads.remove(thread);
        }
        response.response.rect
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn handle_file_drop(
        &mut self,
        ctx: &egui::Context,
        thread: &str,
        group: bool,
        chat: egui::Rect,
        compose: egui::Rect,
        blocked_at_start: bool,
        pal: &ThemePalette,
    ) {
        if self.attachment_drafts.deferred.is_none()
            && !ctx.input(|i| !i.raw.hovered_files.is_empty() || !i.raw.dropped_files.is_empty())
        {
            return;
        }
        let blocked = blocked_at_start || self.file_drop_blocked(ctx);
        let position = self.file_drop_position(ctx);
        let destination = target(position, chat, compose, blocked);
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            ctx.request_repaint_after(Duration::from_millis(33));
            if let Some(target) = destination {
                let (rect, label) = match target {
                    Target::Send => (chat, "Thả để gửi ngay"),
                    Target::Compose => (compose, "Thả để đính kèm · xem lại trước khi gửi"),
                };
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    Id::new("file_drop_hint"),
                ));
                painter.rect_filled(rect, 8.0, pal.primary.linear_multiply(0.16));
                painter.rect_stroke(rect.shrink(1.0), 8.0, Stroke::new(2.0_f32, pal.primary));
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(15.0),
                    pal.text_primary,
                );
            }
        }
        let capture = match self
            .attachment_drafts
            .take_drop(ctx, thread, position, blocked)
        {
            Ok(Some(capture)) => capture,
            Ok(None) => return,
            Err(error) => {
                self.status_msg = error.into();
                return;
            }
        };
        let Some(destination) = target(capture.position, chat, compose, blocked || capture.blocked)
        else {
            self.status_msg =
                "Tệp chưa được gửi: hãy thả vào vùng hội thoại hoặc ô soạn tin".into();
            return;
        };
        let paths = capture.paths;
        if destination == Target::Send {
            let batch = Batch {
                thread: thread.into(),
                group,
                caption: String::new(),
                original: self.settings.send_photo_original,
                files: paths.into_iter().map(|p| (p, None)).collect(),
            };
            if WORKER_TX.get().is_none_or(|tx| {
                tx.send(WorkerCmd::SendDroppedFiles {
                    owner: self.user_uin.clone(),
                    batch,
                })
                .is_err()
            }) {
                self.status_msg = "Không gửi được tệp: bộ xử lý đã dừng".into();
            }
        } else {
            for path in paths {
                let Some(id) = self.attachment_drafts.insert(thread, path.clone()) else {
                    self.status_msg =
                        "Tệp trùng hoặc danh sách chờ đã đầy (16 tệp/hội thoại, 64 tệp tổng)"
                            .into();
                    continue;
                };
                if WORKER_TX.get().is_none_or(|tx| {
                    tx.send(WorkerCmd::PrepareDroppedFile {
                        owner: self.user_uin.clone(),
                        thread: thread.into(),
                        id,
                        path,
                    })
                    .is_err()
                }) {
                    self.attachment_drafts.complete(
                        ctx,
                        &self.user_uin,
                        thread,
                        id,
                        Err("Bộ xử lý đã dừng; bỏ tệp và thả lại để thử".into()),
                    );
                }
            }
            self.composer_focus_pending = true;
        }
    }

    pub(super) fn send_attachment_draft(&mut self, ctx: &egui::Context, thread: &str, group: bool) {
        if !self.is_logged_in || self.attachment_drafts.owner != self.user_uin {
            return;
        }
        if self.file_drop_blocked(ctx) {
            self.status_msg = "Hãy đóng cửa sổ phụ hoặc menu trước khi gửi tệp".into();
            return;
        }
        let Some(items) = self.attachment_drafts.threads.get(thread) else {
            return;
        };
        if items.is_empty() {
            return;
        }
        if !self.composer_mentions.mentions.is_empty()
            || self.replying_to.is_some()
            || self.pending_clipboard_image.is_some()
        {
            self.status_msg =
                "Bỏ mention, hủy trích dẫn hoặc bỏ ảnh clipboard trước khi gửi tệp; tệp chờ vẫn được giữ"
                    .into();
            return;
        }
        if items.iter().any(|i| !matches!(i.result, Some(Ok(_)))) {
            self.status_msg = "Hãy chờ đọc tệp hoặc bỏ tệp bị lỗi trước khi gửi".into();
            return;
        }
        let batch = Batch {
            thread: thread.into(),
            group,
            caption: self.input_message.trim().into(),
            original: self.settings.send_photo_original,
            files: items
                .iter()
                .filter_map(|i| {
                    i.result
                        .as_ref()
                        .and_then(|r| r.as_ref().ok())
                        .map(|stamp| (i.path.clone(), Some(stamp.clone())))
                })
                .collect(),
        };
        if WORKER_TX.get().is_some_and(|tx| {
            tx.send(WorkerCmd::SendDroppedFiles {
                owner: self.user_uin.clone(),
                batch,
            })
            .is_ok()
        }) {
            self.attachment_drafts.threads.remove(thread);
            self.input_message.clear();
            self.composer_mentions.clear();
        } else {
            self.status_msg = "Không gửi được tệp; danh sách chờ vẫn được giữ".into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(std::path::PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "ctdoz-drop-test-{}-{}",
                std::process::id(),
                crate::chat::next_client_message_id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_known_unblocked_drop_zones_can_send() {
        let chat = egui::Rect::from_min_max(egui::pos2(100.0, 40.0), egui::pos2(500.0, 400.0));
        let compose = egui::Rect::from_min_max(egui::pos2(100.0, 400.0), egui::pos2(500.0, 500.0));
        assert_eq!(
            target(Some(chat.center()), chat, compose, false),
            Some(Target::Send)
        );
        assert_eq!(
            target(Some(compose.center()), chat, compose, false),
            Some(Target::Compose)
        );
        assert_eq!(
            target(Some(egui::pos2(200.0, 400.0)), chat, compose, false),
            Some(Target::Compose)
        );
        for position in [
            None,
            Some(egui::pos2(20.0, 250.0)),
            Some(egui::pos2(600.0, 250.0)),
            Some(egui::pos2(300.0, 10.0)),
        ] {
            assert_eq!(target(position, chat, compose, false), None);
        }
        assert_eq!(target(Some(chat.center()), chat, compose, true), None);
        assert_eq!(target(Some(compose.center()), chat, compose, true), None);
    }
    #[test]
    fn staged_files_are_bounded_deduplicated_and_scoped() {
        let mut drafts = Drafts::default();
        drafts.sync_owner("owner-a");
        let old = drafts.insert("thread-a", "one.txt".into()).unwrap();
        assert!(drafts.insert("thread-a", "one.txt".into()).is_none());
        assert_eq!(drafts.len("thread-b"), 0);
        for n in 1..PER_THREAD {
            assert!(drafts
                .insert("thread-a", format!("{n}.txt").into())
                .is_some());
        }
        assert!(drafts.insert("thread-a", "extra.txt".into()).is_none());
        for t in 1..4 {
            for n in 0..PER_THREAD {
                assert!(drafts
                    .insert(&format!("thread-{t}"), format!("{n}.txt").into())
                    .is_some());
            }
        }
        assert!(drafts.insert("thread-b", "extra.txt".into()).is_none());
        drafts.sync_owner("owner-a");
        assert_eq!(drafts.len("thread-a"), PER_THREAD);
        drafts.sync_owner("owner-b");
        assert_eq!(drafts.len("thread-a"), 0);
        let new = drafts.insert("thread-a", "two.txt".into()).unwrap();
        let ctx = egui::Context::default();
        drafts.complete(&ctx, "owner-a", "thread-a", old, Err("late".into()));
        drafts.complete(&ctx, "owner-b", "thread-a", old, Err("removed".into()));
        assert!(drafts.threads["thread-a"][0].result.is_none());
        drafts.remove_thread("owner-a", "thread-a");
        assert_eq!(drafts.len("thread-a"), 1);
        drafts.complete(&ctx, "owner-b", "thread-a", new, Err("new error".into()));
        assert!(matches!(&drafts.threads["thread-a"][0].result,Some(Err(e)) if e=="new error"));
        drafts.remove_thread("owner-b", "thread-a");
        assert_eq!(drafts.len("thread-a"), 0);
    }
    #[tokio::test]
    async fn preview_is_local_bounded_and_missing_or_directory_paths_fail() {
        let temp = Temp::new();
        let image = temp.0.join("photo.png");
        image::RgbaImage::new(128, 32).save(&image).unwrap();
        let preview = prepare(image).await.unwrap();
        let image = preview.image.unwrap();
        assert_eq!(image.dimensions(), (64, 16));
        let text = temp.0.join("file.txt");
        std::fs::write(&text, b"hello").unwrap();
        let preview = prepare(text).await.unwrap();
        assert_eq!(preview.size, 5);
        assert!(preview.image.is_none());
        assert!(prepare(temp.0.clone()).await.is_err());
        assert!(prepare(temp.0.join("missing")).await.is_err());
    }
    #[tokio::test]
    async fn file_changed_after_preview_is_rejected_before_network_send() {
        let temp = Temp::new();
        let path = temp.0.join("file.txt");
        std::fs::write(&path, b"old").unwrap();
        let preview = prepare(path.clone()).await.unwrap();
        std::fs::write(&path, b"new content").unwrap();
        let message = crate::outgoing::pending(
            "file".into(),
            false,
            crate::outgoing::Payload::Attachment {
                path,
                caption: "caption".into(),
                stamp: Some(preview.stamp),
                hd_source: None,
            },
        );
        let session = Session::new(false, false, false).unwrap();
        let error = crate::outgoing::send(&session, "thread-a", &message)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("đã thay đổi"));
    }
}

#[cfg(test)]
mod draft_send_tests {
    use super::*;
    #[test]
    fn reading_failed_or_modal_drafts_do_not_send_or_clear_caption() {
        let ctx = egui::Context::default();
        let mut app = ZaloGuiApp::from_context(
            &ctx,
            Session::new(false, false, false).unwrap(),
            false,
            vec![Thread {
                id: "thread-a".into(),
                ..Default::default()
            }],
            false,
        );
        app.is_logged_in = true;
        app.user_uin = "owner-a".into();
        app.input_message = "caption".into();
        app.attachment_drafts.sync_owner("owner-a");
        let id = app
            .attachment_drafts
            .insert("thread-a", "file.txt".into())
            .unwrap();
        app.send_attachment_draft(&ctx, "thread-a", false);
        assert!(app.status_msg.contains("chờ đọc tệp"));
        assert_eq!(app.attachment_drafts.len("thread-a"), 1);
        assert_eq!(app.input_message, "caption");
        app.attachment_drafts
            .complete(&ctx, "owner-a", "thread-a", id, Err("unreadable".into()));
        app.send_attachment_draft(&ctx, "thread-a", false);
        assert_eq!(app.attachment_drafts.len("thread-a"), 1);
        app.settings_modal_open = true;
        app.send_attachment_draft(&ctx, "thread-a", false);
        assert!(app.status_msg.contains("đóng cửa sổ"));
        assert_eq!(app.input_message, "caption");
    }
}

#[cfg(test)]
mod multipass_tests {
    use super::*;
    #[test]
    fn discarded_layout_keeps_drop_once_without_retargeting() {
        let ctx = egui::Context::default();
        let mut drafts = Drafts::default();
        drafts.sync_owner("owner-a");
        let input = egui::RawInput {
            dropped_files: vec![egui::DroppedFile {
                path: Some("file.txt".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut passes = 0;
        let mut received = 0;
        let _ = ctx.run(input, |ctx| {
            passes += 1;
            if passes == 1 {
                ctx.request_discard("fixture layout changed");
            }
            if let Some(drop) = drafts
                .take_drop(ctx, "thread-a", Some(egui::pos2(20.0, 30.0)), passes == 1)
                .unwrap()
            {
                assert!(drop.blocked);
                assert_eq!(drop.paths, vec![PathBuf::from("file.txt")]);
                received += 1;
            }
        });
        assert_eq!(passes, 2);
        assert_eq!(received, 1);
        assert!(drafts.deferred.is_none());
        let _ = ctx.run(Default::default(), |ctx| {
            assert!(drafts
                .take_drop(ctx, "thread-a", None, false)
                .unwrap()
                .is_none())
        });
    }
    #[test]
    fn a_deferred_drop_cannot_follow_a_thread_or_account_switch() {
        for switch_owner in [false, true] {
            let ctx = egui::Context::default();
            let mut drafts = Drafts::default();
            drafts.sync_owner("owner-a");
            let mut passes = 0;
            let mut received = 0;
            let _ = ctx.run(
                egui::RawInput {
                    dropped_files: vec![egui::DroppedFile {
                        path: Some("file.txt".into()),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                |ctx| {
                    passes += 1;
                    if passes == 1 {
                        ctx.request_discard("fixture");
                    } else if switch_owner {
                        drafts.sync_owner("owner-b");
                    }
                    let thread = if passes > 1 && !switch_owner {
                        "thread-b"
                    } else {
                        "thread-a"
                    };
                    received += usize::from(
                        drafts
                            .take_drop(ctx, thread, None, false)
                            .unwrap()
                            .is_some(),
                    );
                },
            );
            assert_eq!(received, 0);
        }
    }
}
