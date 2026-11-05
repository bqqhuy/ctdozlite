use super::*;

// ---------------------------------------------------------------------------
// Chat Bubble & Sticker Widget Rendering
// ---------------------------------------------------------------------------

/// Request only the visible area and a small scroll-ahead margin, without
/// turning the entire rendered history window into a download queue.
pub(super) fn media_near_viewport(ui: &egui::Ui, height: f32) -> bool {
    let clip = ui.clip_rect().expand(160.0);
    let y = ui.next_widget_position().y;
    clip.intersects(Rect::from_min_max(
        Pos2::new(clip.min.x, y),
        Pos2::new(clip.max.x, y + height),
    ))
}

pub(super) fn is_sticker_message(msg: &ChatMessage) -> Option<(u64, u64)> {
    if let Some(sid) = msg.sticker_id {
        if sid > 0 {
            return Some((sid, msg.sticker_cate_id.unwrap_or(0)));
        }
    }
    if msg.text.trim() == "[Nhãn dán]" {
        return Some((18009, 10130));
    }
    None
}

pub(super) fn render_sticker_widget(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    sticker_textures: &HashMap<String, TextureHandle>,
    loading_stickers: &mut HashSet<String>,
) -> bool {
    let Some((sid, cid)) = is_sticker_message(msg) else {
        return false;
    };

    let key = format!("stk_{cid}_{sid}");
    let url = if cid == 0 && (1..=30).contains(&sid) {
        format!("https://stc-chat.zdn.vn/images/stickers/default/thumb/{sid}.png")
    } else {
        format!("https://api.zaloapp.com/api/emoticon/sticker/webpc?eid={sid}&size=130")
    };

    if media_near_viewport(ui, 128.0)
        && !sticker_textures.contains_key(&key)
        && loading_stickers.insert(key.clone())
    {
        if let Some(tx) = WORKER_TX.get() {
            let _ = tx.send(WorkerCmd::FetchStickerImage {
                key: key.clone(),
                url,
            });
        }
    }

    if let Some(tex) = sticker_textures.get(&key) {
        ui.image((tex.id(), Vec2::new(116.0, 116.0)));
        true
    } else {
        false
    }
}

#[derive(Debug, Clone)]
pub(super) struct CallCardView {
    pub(super) title: String,
    pub(super) status_badge: String,
    pub(super) reason: Option<String>,
    pub(super) is_video: bool,
    pub(super) is_missed_or_failed: bool,
}

/// Detect whether `msg` is a voice/video call log entry and parse its structured metadata.
pub(super) fn extract_call_card_view(_msg: &ChatMessage) -> Option<CallCardView> {
    // Call records are excluded from chat; active calls have their own overlay.
    None
}

pub(super) fn render_call_card_widget(
    ui: &mut egui::Ui,
    card: &CallCardView,
    pal: &ThemePalette,
    card_width: f32,
) -> Option<BubbleAction> {
    let mut action = None;
    let accent = if card.is_missed_or_failed {
        Color32::from_rgb(239, 68, 68)
    } else {
        Color32::from_rgb(34, 197, 94)
    };

    ui.set_width(card_width);
    let text_col_w = (card_width - 52.0).max(110.0);
    ui.vertical(|ui| {
        ui.set_width(card_width);
        ui.horizontal(|ui| {
            let (icon_rect, _) = ui.allocate_exact_size(Vec2::new(40.0, 40.0), Sense::hover());
            ui.painter().circle_filled(
                icon_rect.center(),
                20.0,
                accent.linear_multiply(if pal.is_dark { 0.18 } else { 0.14 }),
            );
            ui.painter().circle_stroke(
                icon_rect.center(),
                20.0,
                Stroke::new(1.2_f32, accent.linear_multiply(0.55)),
            );
            if card.is_video {
                draw_icon_video(ui.painter(), icon_rect.center(), 18.0, accent);
            } else {
                draw_icon_phone(ui.painter(), icon_rect.center(), 18.0, accent);
            }

            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.set_width(text_col_w);
                ui.add(
                    egui::Label::new(
                        RichText::new(&card.title)
                            .size(13.8)
                            .color(pal.text_primary)
                            .strong(),
                    )
                    .wrap(),
                );
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(&card.status_badge)
                            .size(11.8)
                            .color(accent)
                            .strong(),
                    )
                    .wrap(),
                );
                if let Some(reason) = &card.reason {
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(reason).size(11.2).color(pal.text_secondary),
                        )
                        .wrap(),
                    );
                }
            });
        });
        ui.add_space(8.0);
        let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(card_width, 1.0), Sense::hover());
        ui.painter().line_segment(
            [sep_rect.left_center(), sep_rect.right_center()],
            Stroke::new(1.0_f32, pal.border),
        );
        ui.add_space(6.0);

        let btn_label = if card.is_video {
            format!("{}  Gọi video lại", lucide::VIDEO)
        } else {
            format!("{}  Gọi lại", lucide::PHONE)
        };
        let call_btn = egui::Button::new(
            RichText::new(btn_label)
                .size(12.5)
                .color(Color32::WHITE)
                .strong(),
        )
        .fill(pal.primary)
        .stroke(Stroke::new(1.0_f32, pal.primary_hover))
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(card_width, 30.0));

        if ui
            .add(call_btn)
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            action = Some(BubbleAction::CallAgain {
                is_video: card.is_video,
            });
        }
    });
    action
}

/// Open an `http://` or `https://` URL in the user's default system browser.
pub(super) fn open_external_url(url: &str) {
    let _ = crate::platform::open_url(url);
}
/// Native Windows folder picker.
pub(super) fn pick_native_folder_dialog() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        rfd::FileDialog::new()
            .set_title("Chọn thư mục lưu file tải về")
            .pick_folder()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
/// Open a native OS file-picker dialog on a worker thread and return the selected path.
pub(super) fn pick_native_files_dialog(images_only: bool) -> Option<Vec<std::path::PathBuf>> {
    #[cfg(target_os = "windows")]
    {
        // Native Win32 IFileOpenDialog (via `rfd`): opens instantly, no PowerShell /
        // .NET cold start, and runs natively on ARM64. Called from a worker thread,
        // so the blocking API does not freeze the egui event loop.
        let title = if images_only {
            "Chọn hình ảnh để gửi qua Zalo"
        } else {
            "Chọn video hoặc tập tin đính kèm để gửi qua Zalo"
        };
        let mut dlg = rfd::FileDialog::new().set_title(title);
        dlg = if images_only {
            dlg.add_filter("Hình ảnh", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
                .add_filter("Tất cả tập tin", &["*"])
        } else {
            dlg.add_filter("Tất cả tập tin", &["*"])
                .add_filter("Video & âm thanh", &["mp4", "mov", "mkv", "mp3", "wav"])
                .add_filter("Tài liệu", &["pdf", "zip", "docx", "xlsx", "txt"])
        };
        dlg.pick_files()
    }
    #[cfg(not(windows))]
    {
        let _ = images_only;
        None
    }
}

/// Detect `Ctrl+V` or `Shift+Insert` reliably on Windows even when `egui-winit` swallows `Key::V`
/// because the system clipboard contains an image instead of UTF-8 text.
pub(super) fn is_clipboard_paste_triggered(ctx: &egui::Context) -> bool {
    let egui_paste = ctx.input(|i| {
        (i.modifiers.ctrl && i.key_pressed(Key::V))
            || (i.modifiers.shift && i.key_pressed(Key::Insert))
            || i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
    });

    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        extern "system" {
            fn GetAsyncKeyState(v_key: i32) -> i16;
        }
        static WAS_PASTE_DOWN: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);

        let is_focused = ctx.input(|i| i.focused);
        if !is_focused {
            WAS_PASTE_DOWN.store(false, std::sync::atomic::Ordering::SeqCst);
            return egui_paste;
        }
        let (win32_down, modifier_held) = unsafe {
            let ctrl = (GetAsyncKeyState(0x11) as u16 & 0x8000) != 0; // VK_CONTROL
            let v_key = (GetAsyncKeyState(0x56) as u16 & 0x8000) != 0; // 'V'
            let shift = (GetAsyncKeyState(0x10) as u16 & 0x8000) != 0; // VK_SHIFT
            let ins = (GetAsyncKeyState(0x2D) as u16 & 0x8000) != 0; // VK_INSERT
            ((ctrl && v_key) || (shift && ins), ctrl || shift)
        };
        if modifier_held {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        let prev = WAS_PASTE_DOWN.swap(win32_down, std::sync::atomic::Ordering::SeqCst);
        let edge_triggered = win32_down && !prev;
        egui_paste || edge_triggered
    }

    #[cfg(not(target_os = "windows"))]
    {
        egui_paste
    }
}

#[cfg(windows)]
pub(super) fn finalize_clipboard_rgba_image(
    rgba: image::RgbaImage,
) -> Option<(std::path::PathBuf, u32, u32, egui::ColorImage)> {
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    let _ = std::fs::create_dir_all(crate::storage::cache_dir());
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = crate::storage::cache_dir().join(format!("clipboard_{now_ms}.png"));
    rgba.save(&path).ok()?;

    let thumb = if width > 240 || height > 240 {
        image::imageops::thumbnail(&rgba, 240, 240)
    } else {
        rgba
    };
    let (tw, th) = thumb.dimensions();
    let col_img =
        egui::ColorImage::from_rgba_unmultiplied([tw as usize, th as usize], thumb.as_raw());
    Some((path, width, height, col_img))
}

/// Inspect clipboard formats without opening it or decoding pixels on the UI thread.
pub(super) fn clipboard_has_image() -> bool {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn IsClipboardFormatAvailable(format: u32) -> i32;
            fn RegisterClipboardFormatA(name: *const u8) -> u32;
        }
        // SAFETY: constant format IDs and NUL-terminated static strings; no borrowed clipboard memory.
        unsafe {
            [
                8,
                14,
                15,
                17,
                RegisterClipboardFormatA(c"PNG".as_ptr().cast()),
                RegisterClipboardFormatA(c"image/png".as_ptr().cast()),
            ]
            .into_iter()
            .any(|format| format != 0 && IsClipboardFormatAvailable(format) != 0)
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Read an image from the Windows Clipboard supporting:
/// 1. Registered `"PNG"` / `"image/png"` streams (Snipping Tool, ShareX, Chrome/Edge, Discord)
/// 2. `CF_HDROP` (`15`) copied image files from Windows Explorer or ShareX
/// 3. `CF_DIB` (`8`) / `CF_DIBV5` (`17`) 24-bit & 32-bit bitmaps (`BI_RGB` & `BI_BITFIELDS`)
pub(super) fn read_clipboard_image_to_png(
) -> Option<(std::path::PathBuf, u32, u32, egui::ColorImage)> {
    crate::crash_report::phase("clipboard-copy");
    #[cfg(windows)]
    {
        let rgba = crate::clipboard_image::read_windows().ok()??;
        crate::crash_report::phase("clipboard-preview");
        finalize_clipboard_rgba_image(rgba)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Format a URL for clean UI display by stripping tracking query strings (`?_r=1&utm_...`)
/// and truncating to `max_chars`.
pub(super) fn clean_display_url(url: &str, max_chars: usize) -> String {
    let trimmed = url.trim();
    let no_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let base = no_scheme
        .split('?')
        .next()
        .unwrap_or(no_scheme)
        .trim_end_matches('/');
    truncate_chars(base, max_chars).into_owned()
}

/// Extract `(user_comment, url, domain)` from a message when `msg.link_url` is set or when
/// `msg.text` contains an embedded `http://` / `https://` URL.
///
/// Note: `user_comment` only contains extra text typed by the sender *outside* the link's own
/// `link_title` / `link_desc` (so `link_desc` is rendered inside the Link Preview Card rather than
/// duplicated as a large paragraph above the card).
pub(super) fn extract_clean_domain(url: &str) -> String {
    url.split("://")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("Liên kết")
        .trim()
        .strip_prefix("www.")
        .unwrap_or_else(|| {
            url.split("://")
                .nth(1)
                .unwrap_or(url)
                .split('/')
                .next()
                .unwrap_or("Liên kết")
                .trim()
        })
        .to_string()
}

pub(super) fn extract_message_link_view(msg: &ChatMessage) -> Option<(String, String, String)> {
    if msg.is_photo() {
        return None;
    }
    if let Some(url) = msg.link_url.as_deref().filter(|u| !u.is_empty()) {
        let desc_trim = msg.link_desc.as_deref().unwrap_or("").trim();
        let title_trim = msg.link_title.as_deref().unwrap_or("").trim();
        let url_base = url.split('?').next().unwrap_or(url);

        let mut kept_lines = Vec::new();
        for raw_line in msg.text.lines() {
            let mut line = raw_line.trim().to_string();
            if let Some(pos) = line.find("http://").or_else(|| line.find("https://")) {
                let after = &line[pos..];
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                let found = &after[..end];
                if found == url || found.starts_with(url_base) {
                    line = format!("{}{}", &line[..pos], &after[end..])
                        .trim()
                        .to_string();
                }
            }
            if line.is_empty()
                || line == url
                || line == title_trim
                || line == desc_trim
                || line == "[Chia sẻ]"
                || line == "[Liên kết]"
                || (!desc_trim.is_empty() && desc_trim.contains(&line))
                || (!title_trim.is_empty() && title_trim.contains(&line))
            {
                continue;
            }
            kept_lines.push(line);
        }
        let user_comment = kept_lines.join("\n").trim().to_string();
        let domain = extract_clean_domain(url);
        return Some((user_comment, url.to_string(), domain));
    }
    let text = msg.text.trim();
    let pos = text.find("https://").or_else(|| text.find("http://"))?;
    let after = &text[pos..];
    let end = after.find(char::is_whitespace).unwrap_or(after.len());
    let url = after[..end].trim_end_matches([')', ']', ',', ';']);
    if url.len() < 10 {
        return None;
    }
    let caption = format!("{}{}", &text[..pos], &after[end..])
        .trim()
        .to_string();
    let domain = extract_clean_domain(url);
    Some((caption, url.to_string(), domain))
}

/// Normalize a millisecond or second timestamp to milliseconds (`> 0`).
pub(super) fn normalize_ts_ms(ts: i64) -> i64 {
    if ts <= 0 {
        0
    } else if ts < 10_000_000_000 {
        ts * 1000
    } else {
        ts
    }
}

/// Return `true` when a date/time separator pill should be inserted before `curr`.
pub(super) fn should_show_time_divider(prev: Option<&ChatMessage>, curr: &ChatMessage) -> bool {
    let Some(prev) = prev else {
        return true;
    };
    let prev_ms = normalize_ts_ms(prev.ts);
    let curr_ms = normalize_ts_ms(curr.ts);
    prev_ms > 0
        && curr_ms > 0
        && (crate::clock::local_day(prev_ms) != crate::clock::local_day(curr_ms)
            || prev_ms.abs_diff(curr_ms) >= 30 * 60 * 1000)
}

/// Use the same local timezone for dividers and message clocks.
pub(super) fn format_time_divider_label(msg: &ChatMessage) -> String {
    let label = crate::clock::divider_label_at(msg.ts, crate::login::unix_now() * 1000);
    if label.is_empty() {
        if msg.display_time().is_empty() {
            "Cuộc trò chuyện".into()
        } else {
            format!("Hôm nay • {}", msg.time)
        }
    } else {
        label
    }
}

/// Render a centered Zalo-style compact date pill (no side hairlines).
pub(super) fn render_date_divider(ui: &mut egui::Ui, msg: &ChatMessage, pal: &ThemePalette) {
    let label = format_time_divider_label(msg);
    let avail_w = ui.available_width();
    ui.horizontal(|ui| {
        let text_w = ui.fonts(|f| {
            f.layout_no_wrap(
                label.clone(),
                FontId::proportional(11.5),
                pal.text_secondary,
            )
            .size()
            .x
        });
        let pill_w = text_w + 26.0;
        let left_space = ((avail_w - pill_w) * 0.5).max(0.0);
        if left_space > 0.0 {
            ui.add_space(left_space);
        }
        let pill_bg = if pal.is_dark {
            Color32::from_rgba_unmultiplied(30, 34, 41, 235)
        } else {
            Color32::from_rgba_unmultiplied(219, 228, 240, 215)
        };
        egui::Frame::none()
            .fill(pill_bg)
            .rounding(Rounding::same(10.0))
            .inner_margin(Margin::symmetric(13.0, 3.5))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(label)
                        .size(11.4)
                        .color(if pal.is_dark {
                            Color32::from_rgb(215, 218, 224)
                        } else {
                            pal.text_secondary
                        })
                        .strong(),
                );
            });
    });
}

/// Detect whether `msg` is a system/event notification rather than a user speech bubble.
pub(super) fn is_system_notice_message(msg: &ChatMessage) -> bool {
    if msg.id.starts_with("sys_") {
        return true;
    }
    let t = msg.text.trim();
    t.starts_with("[Hệ thống]")
        || t.starts_with("[Thông báo]")
        || (t.starts_with("Bạn và ") && t.contains("đã trở thành bạn bè"))
        || t.contains("đã thêm bạn vào nhóm")
        || t.contains("đã tham gia nhóm")
        || t.contains("đã rời khỏi nhóm")
        || t.contains("đã đổi tên nhóm")
}

/// Render a centered system event pill (e.g., friend connected, group event).
pub(super) fn render_system_notice_pill(ui: &mut egui::Ui, msg: &ChatMessage, pal: &ThemePalette) {
    let avail_w = ui.available_width();
    let text = msg.text.trim();
    ui.horizontal(|ui| {
        let text_w = ui.fonts(|f| {
            f.layout_no_wrap(
                text.to_string(),
                FontId::proportional(12.0),
                pal.text_secondary,
            )
            .size()
            .x
        });
        let pill_w = (text_w + 28.0).min(avail_w - 40.0);
        let left_pad = ((avail_w - pill_w) * 0.5).max(0.0);
        if left_pad > 0.0 {
            ui.add_space(left_pad);
        }
        egui::Frame::none()
            .fill(pal.bg_elevated)
            .stroke(Stroke::new(1.0_f32, pal.border))
            .rounding(Rounding::same(12.0))
            .inner_margin(Margin::symmetric(12.0, 4.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} {text}", lucide::SPARKLES))
                        .size(12.0)
                        .color(pal.text_secondary),
                );
            });
    });
}

/// A stable frame derived from message dimensions, independent of download completion.
fn thumbnail_size(media: &crate::message_view::Media, max_width: f32) -> Vec2 {
    let max_size = Vec2::new(max_width.clamp(40.0, 300.0), 280.0);
    let dimensions = media
        .width
        .zip(media.height)
        .filter(|(w, h)| *w > 0 && *h > 0)
        .map(|(w, h)| Vec2::new(w as f32, h as f32))
        .unwrap_or(Vec2::new(240.0, 160.0));
    let scale = (max_size.x / dimensions.x)
        .min(max_size.y / dimensions.y)
        .min(1.0);
    dimensions * scale
}

/// Return `true` when two adjacent messages belong to the same visual bubble group.
pub(super) fn same_message_cluster(a: &ChatMessage, b: &ChatMessage) -> bool {
    if is_system_notice_message(a) || is_system_notice_message(b) {
        return false;
    }
    if a.is_outgoing != b.is_outgoing || a.sender != b.sender {
        return false;
    }
    if should_show_time_divider(Some(a), b) {
        return false;
    }
    let a_ms = normalize_ts_ms(a.ts);
    let b_ms = normalize_ts_ms(b.ts);
    if a_ms > 0 && b_ms > 0 && (b_ms - a_ms).abs() > 10 * 60 * 1000 {
        return false;
    }
    true
}

// Rendering arguments describe one immediate-mode paint operation.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_message_body(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    thread_name: &str,
    thread_id: &str,
    is_group: bool,
    image_failures: &HashMap<String, crate::media_recovery::Retry>,
    pal: &ThemePalette,
    outgoing: bool,
    max_text_width: f32,
    sticker_textures: &HashMap<String, TextureHandle>,
    loading_stickers: &mut HashSet<String>,
) -> Option<BubbleAction> {
    let mut body_action = None;
    let (text_col, meta_col) = if outgoing {
        (pal.bubble_out_text, pal.bubble_out_meta)
    } else {
        (pal.bubble_in_text, pal.bubble_in_meta)
    };

    let trimmed_text = msg.text.trim();
    if trimmed_text == "[Tin nhắn đã thu hồi]" {
        ui.label(
            RichText::new(format!("{} Tin nhắn đã được thu hồi", lucide::BAN))
                .color(meta_col)
                .italics()
                .size(13.8),
        );
        return None;
    }

    let sender_label = if msg.is_outgoing {
        "Bạn".to_string()
    } else if !msg.sender.is_empty() && msg.sender != "0" {
        msg.sender.clone()
    } else {
        thread_name.to_string()
    };

    let media = crate::message_view::media(msg);
    if matches!(
        media.kind,
        crate::message_view::MediaKind::Photo | crate::message_view::MediaKind::Video
    ) {
        let video = media.kind == crate::message_view::MediaKind::Video;
        let thumb = media.thumbnail.as_deref().unwrap_or_default();
        let key = crate::media_recovery::image_key(thread_id, &msg.id, 0, false);
        let size = thumbnail_size(&media, max_text_width);
        let (width, height) = (size.x, size.y);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
        if !thumb.is_empty()
            && rect.intersects(ui.clip_rect().expand(160.0))
            && !sticker_textures.contains_key(&key)
            && loading_stickers.insert(key.clone())
        {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchChatImage {
                    key: key.clone(),
                    url: thumb.into(),
                    thread: thread_id.into(),
                    group: is_group,
                    message: Box::new(msg.clone()),
                    preview: false,
                });
            }
        }
        ui.painter()
            .rect_filled(rect, Rounding::same(10.0), pal.bg_elevated);
        if let Some(texture) = sticker_textures.get(&key) {
            let size = texture.size_vec2();
            let scale = (rect.width() / size.x).min(rect.height() / size.y).min(1.0);
            let target = Rect::from_center_size(rect.center(), size * scale);
            // Image widgets clip their texture to the same rounded outline as the card.
            ui.put(
                target,
                egui::Image::new(texture)
                    .fit_to_exact_size(target.size())
                    .rounding(10.0),
            );
        } else {
            draw_icon_image(ui.painter(), rect.center(), 24.0, pal.text_muted);
        }
        if video {
            ui.painter()
                .circle_filled(rect.center(), 20.0, Color32::from_black_alpha(150));
            let c = rect.center();
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-5.0, -9.0),
                    c + Vec2::new(10.0, 0.0),
                    c + Vec2::new(-5.0, 9.0),
                ],
                Color32::WHITE,
                Stroke::NONE,
            ));
            if let Some(duration) = media.duration {
                ui.painter().text(
                    rect.right_bottom() - Vec2::new(8.0, 8.0),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("{}:{:02}", duration / 60, duration % 60),
                    FontId::proportional(12.0),
                    Color32::WHITE,
                );
            }
        }
        if let Some(error) = image_failures.get(&key) {
            ui.label(
                RichText::new(error.failure.message())
                    .small()
                    .color(pal.text_muted),
            );
            if ui.small_button(crate::i18n::tr("Thử lại")).clicked() {
                body_action = Some(BubbleAction::RetryMedia { key: key.clone() });
            }
        } else if thumb.is_empty() {
            ui.label(
                RichText::new(crate::i18n::tr("Chưa có thumbnail"))
                    .small()
                    .color(pal.text_muted),
            );
        }
        if response.clicked() {
            if let Some(url) = if video {
                media.original.clone()
            } else {
                media.preview.clone()
            } {
                if video {
                    body_action = Some(BubbleAction::OpenVideo {
                        url,
                        name: media.name.clone(),
                    });
                } else {
                    body_action = Some(BubbleAction::OpenImagePopup {
                        tex_key: key,
                        full_url: url,
                        title: "Hình ảnh".into(),
                        sender: sender_label,
                        time: msg.display_time(),
                    });
                }
            } else {
                body_action = Some(BubbleAction::MediaUnavailable);
            }
        }
        if media.original.is_some() && ui.small_button(crate::i18n::tr("Lưu về máy")).clicked()
        {
            body_action = Some(BubbleAction::SaveAttachment {
                url: media.original.clone().unwrap_or_default(),
                name: media.name.clone(),
            });
        }
        if !video {
            if let Some(caption) = msg.photo_caption() {
                ui.add(egui::Label::new(RichText::new(caption).color(text_col)).wrap());
            }
        }
        return body_action;
    }
    if media.kind == crate::message_view::MediaKind::File {
        ui.add(egui::Label::new(RichText::new(&media.name).strong().color(text_col)).wrap());
        if let Some(url) = media.original {
            if ui
                .button("Mở tệp")
                .on_hover_text(crate::i18n::tr(
                    "Tải vào cache và mở bằng ứng dụng mặc định",
                ))
                .clicked()
            {
                return Some(BubbleAction::OpenAttachment {
                    url,
                    name: media.name,
                });
            }
            if ui.small_button(crate::i18n::tr("Lưu thành…")).clicked() {
                return Some(BubbleAction::SaveAttachment {
                    url,
                    name: media.name,
                });
            }
        } else {
            ui.label(
                RichText::new(crate::i18n::tr("Không còn đường dẫn tải tệp")).color(pal.text_muted),
            );
        }
        return None;
    }

    let Some((caption, url, domain)) = extract_message_link_view(msg) else {
        let mentions =
            crate::rich_text::mention_styles(msg.wire_ref.fields.get("mentions"), &msg.text);
        if msg.text_styles.is_empty() && mentions.is_empty() {
            ui.add(egui::Label::new(RichText::new(&msg.text).color(text_col).size(14.0)).wrap());
        } else {
            let mut styles = msg.text_styles.clone();
            styles.extend(mentions);
            let mention_col = if msg.is_outgoing {
                text_col
            } else {
                pal.primary_hover
            };
            ui.add(
                egui::Label::new(styled_text_job_with_mentions(
                    &msg.text,
                    &styles,
                    text_col,
                    mention_col,
                    14.0,
                ))
                .wrap(),
            );
        }
        return None;
    };

    let link_col = if pal.is_dark {
        Color32::from_rgb(97, 175, 239) // #61AFEF Zalo/Atom clean link blue
    } else {
        pal.primary
    };

    // Auto-enrich link metadata if title or thumbnail is missing/expired
    let needs_meta_enrich = msg.link_title.is_none()
        || msg.link_thumb.is_none()
        || msg
            .link_thumb
            .as_deref()
            .map(|th| {
                th.contains("zpcloud.zdn.vn")
                    && !sticker_textures
                        .contains_key(&format!("thumb_{}", crate::crypto::md5_hex(th)))
            })
            .unwrap_or(false);
    if needs_meta_enrich && media_near_viewport(ui, 300.0) {
        let meta_key = format!("linkmeta_{}", crate::crypto::md5_hex(&url));
        if loading_stickers.insert(meta_key) {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::ParseLinkPreview { url: url.clone() });
            }
        }
    }

    // 1. Optional user comment above the link
    if !caption.is_empty() && caption != url {
        ui.add(egui::Label::new(RichText::new(&caption).color(text_col).size(13.8)).wrap());
        ui.add_space(4.0);
    }

    // 2. Optional compact thumbnail directly inside the bubble (no nested card box)
    if let Some(thumb_url) = msg.link_thumb.as_deref().filter(|u| !u.is_empty()) {
        let key = format!("thumb_{}", crate::crypto::md5_hex(thumb_url));
        if media_near_viewport(ui, 200.0)
            && !sticker_textures.contains_key(&key)
            && (thumb_url.starts_with("http://") || thumb_url.starts_with("https://"))
            && loading_stickers.insert(key.clone())
        {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchStickerImage {
                    key: key.clone(),
                    url: thumb_url.to_string(),
                });
            }
        }
        if let Some(tex) = sticker_textures.get(&key) {
            let [tw, th] = tex.size();
            let aspect = if tw > 0 && th > 0 {
                (th as f32 / tw as f32).clamp(0.45, 0.85)
            } else {
                0.56
            };
            let thumb_w = max_text_width.min(260.0);
            let img_h = (thumb_w * aspect).min(132.0);
            let img = egui::Image::new(tex)
                .fit_to_exact_size(Vec2::new(thumb_w, img_h))
                .rounding(Rounding::same(6.0))
                .sense(Sense::click());
            if ui
                .add(img)
                .on_hover_cursor(CursorIcon::PointingHand)
                .on_hover_text(&url)
                .clicked()
            {
                open_external_url(&url);
            }
            ui.add_space(5.0);
        }
    }

    // 3. Bold Title (exact Zalo PC style: no globe icon, no vertical accent bar, no nested border)
    let header_title = msg
        .link_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty() && *t != domain)
        .unwrap_or_else(|| {
            url.strip_prefix("https://")
                .or_else(|| url.strip_prefix("http://"))
                .unwrap_or(&url)
        });
    let title_resp = ui
        .add(
            egui::Label::new(
                RichText::new(truncate_chars(header_title, 95))
                    .size(13.8)
                    .color(text_col)
                    .strong(),
            )
            .wrap()
            .sense(Sense::click()),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(&url);
    if title_resp.clicked() {
        open_external_url(&url);
    }

    // 4. Optional short description (only if concise and distinct)
    let desc_opt = msg
        .link_desc
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty() && *d != header_title && *d != url && caption.is_empty());
    if let Some(desc) = desc_opt {
        ui.add_space(2.0);
        let d_resp = ui
            .add(
                egui::Label::new(
                    RichText::new(truncate_chars(desc, 110))
                        .size(12.3)
                        .color(meta_col),
                )
                .wrap()
                .sense(Sense::click()),
            )
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text(&url);
        if d_resp.clicked() {
            open_external_url(&url);
        }
    }

    // 5. Clean Domain line (`docs.google.com`, `drive.google.com`, `tiktok.com`)
    ui.add_space(2.5);
    let dom_resp = ui
        .add(
            egui::Label::new(RichText::new(&domain).size(12.6).color(link_col))
                .sense(Sense::click()),
        )
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(&url);
    if dom_resp.clicked() {
        open_external_url(&url);
    }

    body_action
}

// Rendering arguments describe one immediate-mode paint operation.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_chat_bubble(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    prev_same_sender: bool,
    next_same_sender: bool,
    is_latest_outgoing: bool,
    is_group: bool,
    thread_name: &str,
    thread_id: &str,
    image_failures: &HashMap<String, crate::media_recovery::Retry>,
    sender_avatar: Option<&str>,
    pal: &ThemePalette,
    sticker_textures: &HashMap<String, TextureHandle>,
    loading_stickers: &mut HashSet<String>,
    emoji_textures: &HashMap<String, TextureHandle>,
    _idx: usize,
) -> Option<BubbleAction> {
    let avail_w = ui.available_width();
    let max_bubble_width = (avail_w * 0.72).clamp(140.0, 520.0);
    let max_text_width = (max_bubble_width - 24.0).max(90.0);
    let has_sticker = is_sticker_message(msg).is_some();
    let call_card_opt = extract_call_card_view(msg);
    let is_call_card = call_card_opt.is_some();
    let is_photo_card = matches!(
        crate::message_view::media(msg).kind,
        crate::message_view::MediaKind::Photo | crate::message_view::MediaKind::Video
    );
    let is_file_card = matches!(
        msg.link_src.as_deref(),
        Some("Tập tin Zalo") | Some("Video Zalo")
    ) || msg.text.trim().starts_with("[Tập tin]")
        || msg.text.trim().starts_with("[Video]")
        || msg.text.trim().starts_with("[Tin nhắn thoại]");
    let has_link_card = extract_message_link_view(msg).is_some();
    let has_interactive_card = is_call_card || has_link_card || is_photo_card || is_file_card;

    let show_timestamp = !msg.display_time().is_empty()
        && (!next_same_sender
            || is_latest_outgoing
            || has_interactive_card
            || !msg.reactions.is_empty()
            || msg.is_pinned
            || msg.is_starred);

    let has_photo_caption = is_photo_card && msg.photo_caption().is_some();
    let is_frameless_photo = is_photo_card
        && !has_photo_caption
        && msg.reply_to_text.is_none()
        && (msg.is_outgoing || !is_group);

    let sender_is_name = !msg.sender.is_empty()
        && msg.sender != "0"
        && !msg.sender.chars().all(|c| c.is_ascii_digit());
    let show_group_sender = !msg.is_outgoing && !prev_same_sender && is_group && sender_is_name;

    // Pre-measure exact content width for both outgoing and incoming bubbles so every bubble,
    // photo, and call card hugs its content tightly without horizontal overflow or inner/outer frame mismatch.
    let content_width = if has_sticker {
        116.0_f32
    } else if is_call_card {
        max_text_width.min(256.0)
    } else if is_photo_card {
        let media = crate::message_view::media(msg);
        let photo_w = thumbnail_size(&media, max_text_width).x.ceil();
        if has_photo_caption {
            let cap_str = msg
                .link_desc
                .as_deref()
                .filter(|c| !c.trim().is_empty())
                .unwrap_or_else(|| msg.text.trim());
            let cap_w = ui.fonts(|f| {
                f.layout(
                    cap_str.to_string(),
                    FontId::proportional(14.2),
                    Color32::WHITE,
                    max_text_width,
                )
                .size()
                .x
            });
            photo_w.max(cap_w + 4.0).min(max_text_width).ceil()
        } else {
            photo_w
        }
    } else if is_file_card || has_link_card {
        let has_loaded_thumb = msg
            .link_thumb
            .as_deref()
            .filter(|u| !u.is_empty())
            .map(|u| sticker_textures.contains_key(&format!("thumb_{}", crate::crypto::md5_hex(u))))
            .unwrap_or(false);
        if has_loaded_thumb {
            max_text_width.min(260.0)
        } else {
            let (cap, url, dom) = extract_message_link_view(msg).unwrap_or_default();
            let title = msg
                .link_title
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    if !url.is_empty() {
                        &url
                    } else {
                        msg.text.trim()
                    }
                });
            let desc = msg.link_desc.as_deref().unwrap_or("");
            let measure = |s: &str, sz: f32| -> f32 {
                if s.is_empty() {
                    0.0
                } else {
                    ui.fonts(|f| {
                        f.layout(
                            truncate_chars(s, 110).into_owned(),
                            FontId::proportional(sz),
                            Color32::WHITE,
                            max_text_width.min(340.0),
                        )
                        .size()
                        .x
                    })
                }
            };
            let w = measure(title, 13.8)
                .max(measure(&cap, 13.8))
                .max(measure(desc, 12.3))
                .max(measure(&dom, 12.6))
                + 8.0;
            w.clamp(135.0, max_text_width.min(340.0)).ceil()
        }
    } else {
        let display_str = if msg.text.trim() == "[Tin nhắn đã thu hồi]" {
            "  Tin nhắn đã được thu hồi"
        } else {
            msg.text.as_str()
        };
        let text_w = ui.fonts(|f| {
            f.layout(
                display_str.to_string(),
                FontId::proportional(14.2),
                Color32::WHITE,
                max_text_width,
            )
            .size()
            .x
        });
        let quote_w = match msg.reply_to_text.as_ref() {
            Some(qt) => {
                let q_body_w = ui.fonts(|f| {
                    f.layout(
                        truncate_chars(qt, 80).into_owned(),
                        FontId::proportional(12.0),
                        Color32::WHITE,
                        (max_text_width - 28.0).max(80.0),
                    )
                    .size()
                    .x
                });
                let who = msg
                    .reply_to_sender
                    .as_deref()
                    .filter(|sn| !sn.is_empty() && !sn.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or("Tin nhắn trích dẫn");
                let q_who_w = ui.fonts(|f| {
                    f.layout_no_wrap(who.to_string(), FontId::proportional(11.5), Color32::WHITE)
                        .size()
                        .x
                });
                q_body_w.max(q_who_w) + 30.0
            }
            None => 0.0_f32,
        };
        let sender_w = if show_group_sender {
            ui.fonts(|f| {
                f.layout_no_wrap(
                    truncate_chars(&msg.sender, 22).into_owned(),
                    FontId::proportional(12.0),
                    Color32::WHITE,
                )
                .size()
                .x
            }) + 8.0
        } else {
            0.0_f32
        };
        let marks_w = if !msg.reactions.is_empty() || msg.is_pinned || msg.is_starred {
            78.0_f32
        } else {
            0.0_f32
        };
        let ts_min_w = if show_timestamp && is_latest_outgoing && msg.is_outgoing {
            96.0_f32
        } else {
            44.0_f32
        };
        text_w
            .max(quote_w)
            .max(sender_w)
            .max(marks_w)
            .max(ts_min_w)
            .min(max_text_width)
            .ceil()
            + 4.0
    };

    let h_margin = if has_sticker || is_frameless_photo {
        2.0_f32
    } else {
        12.0_f32
    };
    let v_margin = if has_sticker || is_frameless_photo {
        2.0_f32
    } else {
        8.5_f32
    };
    let total_bubble_w = content_width + h_margin * 2.0;

    if msg.is_outgoing {
        let out_resp = ui.vertical(|ui| {
            let row_resp = ui.horizontal(|ui| {
                let left_pad = (ui.available_width() - total_bubble_w - 6.0).max(0.0);
                if left_pad > 0.0 {
                    ui.add_space(left_pad);
                }

                let fill = if has_sticker || is_frameless_photo {
                    Color32::TRANSPARENT
                } else if let Some(card) = &call_card_opt {
                    if card.is_missed_or_failed {
                        if pal.is_dark {
                            Color32::from_rgb(38, 23, 30)
                        } else {
                            Color32::from_rgb(254, 242, 242)
                        }
                    } else {
                        pal.bg_elevated
                    }
                } else {
                    pal.bubble_out_bg
                };
                let stroke = if has_sticker || is_frameless_photo {
                    Stroke::NONE
                } else if let Some(card) = &call_card_opt {
                    let border_col = if card.is_missed_or_failed {
                        Color32::from_rgb(239, 68, 68).linear_multiply(0.55)
                    } else {
                        Color32::from_rgb(34, 197, 94).linear_multiply(0.55)
                    };
                    Stroke::new(1.2_f32, border_col)
                } else if !pal.is_dark {
                    Stroke::new(1.0_f32, Color32::from_rgb(186, 214, 255))
                } else {
                    Stroke::new(1.0_f32, Color32::from_rgb(53, 76, 104))
                };
                let rounding = Rounding::same(8.0);

                let mut card_action = None;
                let mut quote_action = None;
                let fr = egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(rounding)
                    .inner_margin(Margin::symmetric(h_margin, v_margin))
                    .show(ui, |ui| {
                        ui.set_min_width(content_width);
                        ui.set_max_width(content_width);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            quote_action = render_quote_block(ui, msg, pal, true, content_width);
                            if let Some(card) = &call_card_opt {
                                card_action = render_call_card_widget(ui, card, pal, content_width);
                            } else if !render_sticker_widget(
                                ui,
                                msg,
                                sticker_textures,
                                loading_stickers,
                            ) {
                                card_action = render_message_body(
                                    ui,
                                    msg,
                                    thread_name,
                                    thread_id,
                                    is_group,
                                    image_failures,
                                    pal,
                                    true,
                                    content_width,
                                    sticker_textures,
                                    loading_stickers,
                                );
                            }
                            if let Some(action) = render_reactions(ui, msg, pal, emoji_textures) {
                                card_action = Some(action);
                            }
                            if let Some(job) = &msg.outgoing_job {
                                if msg.delivery == crate::message_model::DeliveryState::Failed {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(
                                            Color32::from_rgb(220, 60, 60),
                                            "⚠ Gửi lỗi",
                                        )
                                        .on_hover_text(&job.error);
                                        if ui.button(crate::i18n::tr("Gửi lại")).clicked() {
                                            card_action = Some(BubbleAction::RetrySend);
                                        }
                                    });
                                    ui.label(RichText::new(&job.error).size(11.0));
                                } else if msg.delivery
                                    == crate::message_model::DeliveryState::Pending
                                {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(crate::i18n::tr("Đang gửi / chờ tải lên…"));
                                    });
                                }
                            }
                            if show_timestamp && !has_sticker && !is_frameless_photo {
                                ui.add_space(3.5);
                                let ts_txt = if is_latest_outgoing {
                                    match msg.delivery {
                                        crate::message_model::DeliveryState::ServerAccepted => {
                                            format!("{} · Đã gửi", msg.display_time())
                                        }
                                        crate::message_model::DeliveryState::Delivered => {
                                            format!("{} · Đã nhận", msg.display_time())
                                        }
                                        crate::message_model::DeliveryState::Read => {
                                            format!("{} · Đã xem", msg.display_time())
                                        }
                                        crate::message_model::DeliveryState::Pending => {
                                            format!("{} · Đang gửi", msg.display_time())
                                        }
                                        crate::message_model::DeliveryState::Failed => {
                                            format!("{} · Gửi lỗi", msg.display_time())
                                        }
                                        _ => msg.display_time(),
                                    }
                                } else {
                                    msg.display_time()
                                };
                                ui.label(
                                    RichText::new(ts_txt).size(11.0).color(pal.bubble_out_meta),
                                );
                            }
                        });
                    });
                if fr.response.double_clicked() {
                    return Some(BubbleAction::DoubleClickReply);
                }
                quote_action.or(card_action)
            });

            row_resp.inner
        });
        out_resp.inner
    } else {
        ui.horizontal_top(|ui| {
            let display_sender = if sender_is_name {
                &msg.sender
            } else {
                thread_name
            };
            let sender_accent = avatar_color_for(display_sender, false);

            // Sender Avatar (only on first message of a consecutive group)
            if !prev_same_sender {
                let (av_rect, _) = ui.allocate_exact_size(Vec2::new(30.0, 30.0), Sense::hover());
                paint_user_avatar(
                    ui.painter(),
                    av_rect.center(),
                    15.0,
                    display_sender,
                    false,
                    sender_avatar,
                    sticker_textures,
                    loading_stickers,
                );
            } else {
                ui.add_space(30.0);
            }

            let fill = if has_sticker || is_frameless_photo {
                Color32::TRANSPARENT
            } else if let Some(card) = &call_card_opt {
                if card.is_missed_or_failed {
                    if pal.is_dark {
                        Color32::from_rgb(38, 23, 30)
                    } else {
                        Color32::from_rgb(254, 242, 242)
                    }
                } else {
                    pal.bg_elevated
                }
            } else {
                pal.bubble_in_bg
            };
            let stroke = if has_sticker || is_frameless_photo {
                Stroke::NONE
            } else if let Some(card) = &call_card_opt {
                let border_col = if card.is_missed_or_failed {
                    Color32::from_rgb(239, 68, 68).linear_multiply(0.55)
                } else {
                    Color32::from_rgb(34, 197, 94).linear_multiply(0.55)
                };
                Stroke::new(1.2_f32, border_col)
            } else {
                Stroke::new(1.0_f32, pal.bubble_in_border)
            };
            let rounding = Rounding::same(8.0);

            ui.vertical(|ui| {
                ui.set_max_width(total_bubble_w);
                let mut card_action = None;
                let mut quote_action = None;
                let fr = egui::Frame::none()
                    .fill(fill)
                    .stroke(stroke)
                    .rounding(rounding)
                    .inner_margin(Margin::symmetric(h_margin, v_margin))
                    .show(ui, |ui| {
                        ui.set_min_width(content_width);
                        ui.set_max_width(content_width);
                        ui.with_layout(Layout::top_down(Align::Min), |ui| {
                            if show_group_sender {
                                ui.label(
                                    RichText::new(truncate_chars(&msg.sender, 22))
                                        .color(sender_accent)
                                        .size(12.0)
                                        .strong(),
                                );
                                ui.add_space(2.0);
                            }
                            quote_action = render_quote_block(ui, msg, pal, false, content_width);
                            if let Some(card) = &call_card_opt {
                                card_action = render_call_card_widget(ui, card, pal, content_width);
                            } else if !render_sticker_widget(
                                ui,
                                msg,
                                sticker_textures,
                                loading_stickers,
                            ) {
                                card_action = render_message_body(
                                    ui,
                                    msg,
                                    thread_name,
                                    thread_id,
                                    is_group,
                                    image_failures,
                                    pal,
                                    false,
                                    content_width,
                                    sticker_textures,
                                    loading_stickers,
                                );
                            }
                            if let Some(action) = render_reactions(ui, msg, pal, emoji_textures) {
                                card_action = Some(action);
                            }
                            if let Some(job) = &msg.outgoing_job {
                                if msg.delivery == crate::message_model::DeliveryState::Failed {
                                    ui.horizontal(|ui| {
                                        ui.colored_label(
                                            Color32::from_rgb(220, 60, 60),
                                            "⚠ Gửi lỗi",
                                        )
                                        .on_hover_text(&job.error);
                                        if ui.button(crate::i18n::tr("Gửi lại")).clicked() {
                                            card_action = Some(BubbleAction::RetrySend);
                                        }
                                    });
                                    ui.label(RichText::new(&job.error).size(11.0));
                                } else if msg.delivery
                                    == crate::message_model::DeliveryState::Pending
                                {
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(crate::i18n::tr("Đang gửi / chờ tải lên…"));
                                    });
                                }
                            }
                            if show_timestamp && !has_sticker && !is_frameless_photo {
                                ui.add_space(3.5);
                                ui.label(
                                    RichText::new(msg.display_time())
                                        .color(pal.bubble_in_meta)
                                        .size(11.0),
                                );
                            }
                        });
                    });
                if fr.response.double_clicked() {
                    return Some(BubbleAction::DoubleClickReply);
                }
                quote_action.or(card_action)
            })
            .inner
        })
        .inner
    }
}

#[derive(Clone, PartialEq)]
pub(super) enum BubbleAction {
    OpenAttachment {
        url: String,
        name: String,
    },
    SaveAttachment {
        url: String,
        name: String,
    },
    OpenVideo {
        url: String,
        name: String,
    },
    RetryMedia {
        key: String,
    },
    MediaUnavailable,
    RetrySend,
    DoubleClickReply,
    Reply,
    Mention,
    JumpToQuoted(String),
    Forward,
    React(&'static str),
    ShowReactors,
    Reminder,
    PinOnServer,
    TogglePin,
    ToggleStar,
    DeleteForMe,
    RecallForEveryone,
    CallAgain {
        is_video: bool,
    },
    OpenImagePopup {
        tex_key: String,
        full_url: String,
        title: String,
        sender: String,
        time: String,
    },
}

/// Right-click menu on a chat bubble: reactions, reply, forward, copy, delete-for-me.
pub(super) fn bubble_context_menu(
    ui: &mut egui::Ui,
    bubble: &egui::Response,
    msg: &ChatMessage,
    emoji_textures: &HashMap<String, TextureHandle>,
    is_group: bool,
    protected: bool,
) -> Option<BubbleAction> {
    let secondary_or_popup = ui.input(|i| {
        i.pointer.button_down(egui::PointerButton::Secondary)
            || i.pointer.button_clicked(egui::PointerButton::Secondary)
            || i.pointer.button_released(egui::PointerButton::Secondary)
    }) || ui.memory(|m| m.any_popup_open());
    if !secondary_or_popup {
        return None;
    }
    let resp = ui.interact(
        bubble.rect,
        ui.id().with(("bubble_ctx", msg.ts, &msg.id)),
        Sense::click(),
    );
    let mut action = None;

    resp.context_menu(|ui| {
        ui.set_min_width(210.0);
        if is_group
            && !msg.is_outgoing
            && msg.wire_ref.sender.as_ref().is_some_and(|v| {
                v.as_u64().is_some()
                    || v.as_str()
                        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            })
            && ui
                .button(crate::i18n::tr("@ Nhắc đến người gửi (trong nhóm)"))
                .clicked()
        {
            action = Some(BubbleAction::Mention);
            ui.close_menu();
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            for &(code, key, tooltip) in &REACTION_LIST {
                if let Some(tex) = emoji_textures.get(key) {
                    let img = egui::Image::new(tex).fit_to_exact_size(Vec2::splat(24.0));
                    let btn = egui::ImageButton::new(img).rounding(Rounding::same(6.0));
                    if ui.add(btn).on_hover_text(tooltip).clicked() {
                        action = Some(BubbleAction::React(code));
                        ui.close_menu();
                    }
                } else if ui.button(RichText::new(code).size(18.0)).clicked() {
                    action = Some(BubbleAction::React(code));
                    ui.close_menu();
                }
            }
        });
        ui.separator();
        if (msg.link_src.as_deref() == Some("Hình ảnh Zalo")
            || msg.text.trim() == "[Hình ảnh]"
            || msg.text.trim().starts_with("[Hình ảnh] "))
            && msg
                .link_thumb
                .as_deref()
                .or(msg.link_url.as_deref())
                .map(|u| !u.is_empty())
                .unwrap_or(false)
        {
            let thumb_url = msg
                .link_thumb
                .as_deref()
                .or(msg.link_url.as_deref())
                .unwrap_or_default();
            if ui
                .button(format!("{} Xem ảnh phóng to", lucide::SEARCH))
                .clicked()
            {
                action = Some(BubbleAction::OpenImagePopup {
                    tex_key: format!("thumb_{}", crate::crypto::md5_hex(thumb_url)),
                    full_url: msg
                        .link_url
                        .clone()
                        .unwrap_or_else(|| thumb_url.to_string()),
                    title: msg
                        .link_title
                        .clone()
                        .unwrap_or_else(|| "Hình ảnh".to_string()),
                    sender: if msg.is_outgoing {
                        "Bạn".to_string()
                    } else {
                        msg.sender.clone()
                    },
                    time: msg.display_time(),
                });
                ui.close_menu();
            }
        }
        if ui.button(crate::i18n::tr("Trả lời")).clicked() {
            action = Some(BubbleAction::Reply);
            ui.close_menu();
        }
        if ui
            .add_enabled(
                !protected && crate::chat::can_forward_message(msg),
                egui::Button::new(crate::i18n::tr("Chuyển tiếp")),
            )
            .on_disabled_hover_text(
                "Chưa hỗ trợ chuyển tiếp loại tin này; hãy tải và gửi lại tệp gốc",
            )
            .clicked()
        {
            action = Some(BubbleAction::Forward);
            ui.close_menu();
        }
        if is_group
            && ui
                .button(crate::i18n::tr("Ghim trên mọi thiết bị"))
                .clicked()
        {
            action = Some(BubbleAction::PinOnServer);
            ui.close_menu();
        }
        if ui
            .button(crate::i18n::tr("Tạo nhắc hẹn trên Zalo…"))
            .clicked()
        {
            action = Some(BubbleAction::Reminder);
            ui.close_menu();
        }
        if ui
            .add_enabled(
                !protected,
                egui::Button::new(crate::i18n::tr("Copy tin nhắn")),
            )
            .clicked()
        {
            ui.ctx().copy_text(msg.text.clone());
            ui.close_menu();
        }
        if ui
            .button(if msg.is_pinned {
                "Bỏ ghim trên máy này"
            } else {
                "Ghim trên máy này"
            })
            .clicked()
        {
            action = Some(BubbleAction::TogglePin);
            ui.close_menu();
        }
        if ui
            .button(if msg.is_starred {
                "Bỏ đánh dấu tin nhắn"
            } else {
                "Đánh dấu tin nhắn"
            })
            .clicked()
        {
            action = Some(BubbleAction::ToggleStar);
            ui.close_menu();
        }
        ui.separator();
        if msg.is_outgoing
            && msg.text.trim() != "[Tin nhắn đã thu hồi]"
            && ui
                .button(
                    RichText::new(crate::i18n::tr("Thu hồi với mọi người"))
                        .color(Color32::from_rgb(249, 115, 22)),
                )
                .clicked()
        {
            action = Some(BubbleAction::RecallForEveryone);
            ui.close_menu();
        }
        if ui
            .button(
                RichText::new(crate::i18n::tr("Xóa chỉ ở phía tôi"))
                    .color(Color32::from_rgb(220, 60, 60)),
            )
            .clicked()
        {
            action = Some(BubbleAction::DeleteForMe);
            ui.close_menu();
        }
    });
    action
}

/// Quoted-message block shown at the top of a reply bubble.
pub(super) fn render_quote_block(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    pal: &ThemePalette,
    outgoing: bool,
    max_w: f32,
) -> Option<BubbleAction> {
    let qt = msg.reply_to_text.as_ref()?;
    let (text_col, meta_col) = if outgoing {
        (pal.bubble_out_text, pal.bubble_out_meta)
    } else {
        (pal.bubble_in_text, pal.bubble_in_meta)
    };
    let bar_col = if outgoing && pal.is_dark {
        Color32::from_rgb(147, 197, 253)
    } else {
        pal.primary
    };
    let bg_col = if outgoing && pal.is_dark {
        Color32::from_rgba_unmultiplied(0, 0, 0, 42)
    } else if outgoing {
        Color32::from_rgba_unmultiplied(255, 255, 255, 150)
    } else {
        text_col.linear_multiply(0.06)
    };
    let inner_col_w = (max_w - 28.0).max(72.0);
    let frame = egui::Frame::none()
        .fill(bg_col)
        .rounding(Rounding::same(7.0))
        .inner_margin(Margin::symmetric(8.0, 5.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (bar_rect, _) = ui.allocate_exact_size(Vec2::new(3.0, 28.0), Sense::hover());
                ui.painter()
                    .rect_filled(bar_rect, Rounding::same(1.5), bar_col);
                ui.vertical(|ui| {
                    ui.set_width(inner_col_w);
                    let who = msg
                        .reply_to_sender
                        .as_deref()
                        .filter(|sn| !sn.is_empty() && !sn.chars().all(|c| c.is_ascii_digit()))
                        .unwrap_or("Tin nhắn trích dẫn");
                    ui.label(RichText::new(who).color(bar_col).size(11.5).strong());
                    ui.add(
                        egui::Label::new(
                            RichText::new(truncate_chars(qt, 84))
                                .color(meta_col)
                                .size(12.0),
                        )
                        .wrap(),
                    );
                });
            });
        });
    let reference = crate::message_navigation::message_reply_reference(msg);
    let response = ui.interact(
        frame.response.rect,
        frame.response.id.with("quote_jump"),
        Sense::click(),
    );
    let clicked = response
        .on_hover_cursor(CursorIcon::PointingHand)
        .on_hover_text(crate::i18n::tr("Đi tới tin nhắn gốc đã tải"))
        .clicked();
    ui.add_space(5.0);
    clicked.then(|| BubbleAction::JumpToQuoted(reference.unwrap_or_default()))
}

pub(super) fn render_reactions(
    ui: &mut egui::Ui,
    msg: &ChatMessage,
    pal: &ThemePalette,
    emoji_textures: &HashMap<String, TextureHandle>,
) -> Option<BubbleAction> {
    let has_pin = msg.is_pinned;
    let has_star = msg.is_starred;
    let has_react = !msg.reactions.is_empty() || !msg.reaction_summaries.is_empty();

    if !has_pin && !has_star && !has_react {
        return None;
    }

    let mut action = None;
    ui.add_space(3.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4.0, 2.0);

        if has_pin {
            egui::Frame::none()
                .fill(pal.primary.linear_multiply(0.18))
                .stroke(Stroke::new(1.0_f32, pal.primary))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::symmetric(5.0, 2.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{} Đã ghim", lucide::PIN))
                            .size(11.0)
                            .color(pal.primary)
                            .strong(),
                    );
                });
        }

        if has_star {
            egui::Frame::none()
                .fill(Color32::from_rgb(234, 179, 8).linear_multiply(0.2))
                .stroke(Stroke::new(1.0_f32, Color32::from_rgb(234, 179, 8)))
                .rounding(Rounding::same(8.0))
                .inner_margin(Margin::symmetric(5.0, 2.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("{} Đã lưu", lucide::STAR))
                            .size(11.0)
                            .color(Color32::from_rgb(234, 179, 8))
                            .strong(),
                    );
                });
        }

        if has_react {
            egui::Frame::none()
                .fill(pal.bg_elevated)
                .stroke(Stroke::new(1.0_f32, pal.border))
                .rounding(Rounding::same(12.0))
                .inner_margin(Margin::symmetric(6.0, 2.0))
                .show(ui, |ui| {
                    if ui.small_button("Xem người thả").clicked() {
                        action = Some(BubbleAction::ShowReactors);
                    }
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                        let reactions: Vec<(String, u64)> = if msg.reaction_summaries.is_empty() {
                            msg.reactions.iter().map(|r| (r.clone(), 1)).collect()
                        } else {
                            msg.reaction_summaries
                                .iter()
                                .map(|r| (r.emoji.clone(), r.count))
                                .collect()
                        };
                        for (r, count) in &reactions {
                            let key = emoji_to_tex_key(r);
                            if let Some(tex) = emoji_textures.get(key) {
                                ui.add(egui::Image::new(tex).fit_to_exact_size(Vec2::splat(18.0)));
                            } else {
                                ui.label(RichText::new(r).size(13.0));
                            }
                            if *count > 1 {
                                ui.small(count.to_string());
                            }
                        }
                    });
                });
        }
    });
    action
}

#[cfg(test)]
mod quote_navigation_tests {
    use super::*;

    #[test]
    fn quote_navigation_bubble_propagates_action_for_both_directions() {
        for is_outgoing in [false, true] {
            let ctx = egui::Context::default();
            let msg = ChatMessage {
                id: "reply-1".into(),
                text: "Reply body".into(),
                sender: "Sender".into(),
                reply_to_id: Some("original-1".into()),
                reply_to_text: Some("Original text".into()),
                is_outgoing,
                ..Default::default()
            };
            let mut position = egui::pos2(30.0, 30.0);
            let mut action = None;
            for frame in 0..4 {
                let mut events = vec![egui::Event::PointerMoved(position)];
                if frame >= 2 {
                    events.push(egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 2,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(600.0, 400.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            action = render_chat_bubble(
                                ui,
                                &msg,
                                false,
                                false,
                                false,
                                false,
                                "Thread",
                                "thread-id",
                                &HashMap::new(),
                                None,
                                &ThemePalette::light(),
                                &HashMap::new(),
                                &mut HashSet::new(),
                                &HashMap::new(),
                                0,
                            );
                        });
                    },
                );
                if frame == 0 {
                    position = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.job.text == "Original text" => {
                                Some(text.pos + Vec2::new(4.0, 4.0))
                            }
                            _ => None,
                        })
                        .expect("Quote must be rendered in the bubble");
                }
            }
            assert!(
                matches!(action, Some(BubbleAction::JumpToQuoted(reference)) if reference == "original-1")
            );
        }
    }

    #[test]
    fn quote_navigation_emits_exact_reference_when_clicked() {
        let ctx = egui::Context::default();
        let msg = ChatMessage {
            reply_to_id: Some("00123".into()),
            reply_to_text: Some("Original text".into()),
            ..Default::default()
        };
        let mut action = None;
        for pressed in [None, Some(true), Some(false)] {
            let mut events = vec![egui::Event::PointerMoved(egui::pos2(30.0, 30.0))];
            if let Some(pressed) = pressed {
                events.push(egui::Event::PointerButton {
                    pos: egui::pos2(30.0, 30.0),
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        action = render_quote_block(ui, &msg, &ThemePalette::light(), false, 300.0);
                    });
                },
            );
        }
        assert!(
            matches!(action, Some(BubbleAction::JumpToQuoted(reference)) if reference == "00123")
        );
    }

    #[test]
    fn quote_navigation_advertises_click_when_reference_exists() {
        let ctx = egui::Context::default();
        let msg = ChatMessage {
            reply_to_id: Some("original-1".into()),
            reply_to_text: Some("Original text".into()),
            ..Default::default()
        };
        for _ in 0..2 {
            let output = ctx.run(
                egui::RawInput {
                    events: vec![egui::Event::PointerMoved(egui::pos2(30.0, 30.0))],
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        render_quote_block(ui, &msg, &ThemePalette::light(), false, 300.0);
                    });
                },
            );
            if output.platform_output.cursor_icon == egui::CursorIcon::PointingHand {
                return;
            }
        }
        panic!("Quote with an exact reference must advertise a clickable target");
    }
}

#[cfg(test)]
mod photo_viewer_tests {
    use super::*;

    #[test]
    fn photo_click_opens_viewer_and_transport_url_is_never_painted() {
        for is_outgoing in [false, true] {
            let ctx = egui::Context::default();
            let url = "https://photo-stal-16.zdn.vn/fixture/photo.jpg";
            // Exercise old cached link metadata as well as the new photo classifier.
            let msg = ChatMessage {
                id: "photo-fixture".into(),
                kind: crate::message_model::MessageKind::Link,
                text: url.into(),
                link_url: Some(url.into()),
                link_title: Some(url.into()),
                is_outgoing,
                ..Default::default()
            };
            let texture = ctx.load_texture(
                "fixture",
                egui::ColorImage::new([160, 120], Color32::LIGHT_BLUE),
                Default::default(),
            );
            let textures =
                HashMap::from([(format!("thumb_{}", crate::crypto::md5_hex(url)), texture)]);
            let mut action = None;
            for frame in 0..4 {
                let position = egui::pos2(30.0, 30.0);
                let mut events = vec![egui::Event::PointerMoved(position)];
                if frame >= 2 {
                    events.push(egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: frame == 2,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            Vec2::new(600.0, 400.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            action = render_message_body(
                                ui,
                                &msg,
                                "Fixture",
                                "thread-id",
                                false,
                                &HashMap::new(),
                                &ThemePalette::light(),
                                is_outgoing,
                                300.0,
                                &textures,
                                &mut HashSet::new(),
                            );
                        });
                    },
                );
                for shape in output.shapes {
                    if let egui::Shape::Text(text) = shape.shape {
                        assert!(!text.galley.job.text.contains("zdn.vn"));
                    }
                }
            }
            assert!(
                matches!(action, Some(BubbleAction::OpenImagePopup { full_url, title, .. }) if full_url == url && title == "Hình ảnh")
            );
        }
    }
}

#[cfg(test)]
mod media_visibility_tests {
    use super::*;
    #[test]
    fn offscreen_sticker_does_not_start_download_until_it_is_visible() {
        let ctx = egui::Context::default();
        let msg = ChatMessage {
            sticker_id: Some(1),
            sticker_cate_id: Some(0),
            ..Default::default()
        };
        let textures = HashMap::new();
        let mut loading = HashSet::new();
        let input = || egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0))),
            ..Default::default()
        };
        let _ = ctx.run(input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(2000.0);
                render_sticker_widget(ui, &msg, &textures, &mut loading);
            });
        });
        assert!(
            loading.is_empty(),
            "offscreen history must not flood the image queue"
        );
        let _ = ctx.run(input(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_sticker_widget(ui, &msg, &textures, &mut loading);
            });
        });
        assert!(loading.contains("stk_0_1"));
    }
}

/// Lay out rich-text spans; egui ships no bold face, so bold is a slightly larger glyph size.
#[cfg(test)]
pub(super) fn styled_text_job(
    text: &str,
    styles: &[crate::rich_text::TextStyle],
    color: Color32,
    size: f32,
) -> egui::text::LayoutJob {
    styled_text_job_with_mentions(text, styles, color, color, size)
}

/// Like [`styled_text_job`], drawing mention ranges in `mention_color` and slightly larger.
pub(super) fn styled_text_job_with_mentions(
    text: &str,
    styles: &[crate::rich_text::TextStyle],
    color: Color32,
    mention_color: Color32,
    size: f32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    for (range, style) in crate::rich_text::segments(text, styles) {
        let color = if style.mention { mention_color } else { color };
        let stroke = egui::Stroke::new(1.0_f32, color);
        job.append(
            &text[range],
            0.0,
            egui::TextFormat {
                font_id: egui::FontId::proportional(if style.bold || style.mention {
                    size + 0.6
                } else {
                    size
                }),
                color,
                italics: style.italic,
                underline: if style.underline {
                    stroke
                } else {
                    egui::Stroke::NONE
                },
                strikethrough: if style.strike {
                    stroke
                } else {
                    egui::Stroke::NONE
                },
                ..Default::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod rich_text_tests {
    use super::*;

    #[test]
    fn styled_job_keeps_exact_text_and_marks_styled_sections() {
        let styles = [crate::rich_text::TextStyle {
            start: 4,
            len: 3,
            underline: true,
            italic: true,
            ..Default::default()
        }];
        let job = styled_text_job("Xin đậm nhé", &styles, Color32::WHITE, 14.0);
        assert_eq!(job.text, "Xin đậm nhé");
        let marked: Vec<&str> = job
            .sections
            .iter()
            .filter(|s| s.format.italics)
            .map(|s| &job.text[s.byte_range.clone()])
            .collect();
        assert_eq!(marked, ["đậm"]);
    }
}

#[cfg(test)]
mod thumbnail_geometry_tests {
    use super::*;
    #[test]
    fn frames_preserve_aspect_ratio_without_upscaling_small_images() {
        for (w, h) in [(4000, 500), (500, 4000), (80, 60)] {
            let m = crate::message_view::Media {
                width: Some(w),
                height: Some(h),
                ..Default::default()
            };
            let size = thumbnail_size(&m, 270.0);
            assert!(size.x <= 270.0 && size.y <= 280.0);
            assert!((size.x / size.y - w as f32 / h as f32).abs() < 0.01);
            if w == 80 {
                assert_eq!(size, Vec2::new(80.0, 60.0));
            }
        }
    }
}

/// Native controls on a stable message identity; the row-to-toolbar corridor avoids hover gaps.
pub(super) fn hover_message_actions(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    thread: &str,
    msg: &ChatMessage,
    emoji: &HashMap<String, TextureHandle>,
) -> Option<BubbleAction> {
    if !ui.is_rect_visible(rect) || is_system_notice_message(msg) || msg.id.is_empty() {
        return None;
    }
    let key = Id::new(("hover_message", thread));
    let identity = (&msg.id, msg.ts);
    let bar = egui::Rect::from_min_size(
        rect.right_top() - Vec2::new(88.0, 0.0),
        Vec2::new(88.0, 34.0),
    );
    let active = ui
        .ctx()
        .data(|d| d.get_temp::<(String, i64, egui::Rect)>(key));
    let pointer = ui.input(|i| i.pointer.hover_pos());
    let over_active_bar = active
        .as_ref()
        .is_some_and(|(_, _, bar)| pointer.is_some_and(|p| bar.expand(3.0).contains(p)));
    let this_active = active
        .as_ref()
        .is_some_and(|(id, ts, _)| (id, *ts) == (identity.0, identity.1));
    let popup_open = ui.memory(|m| m.any_popup_open());
    if (over_active_bar || popup_open) && !this_active {
        return None;
    }
    if !(this_active && popup_open)
        && !pointer.is_some_and(|p| rect.contains(p))
        && !(this_active
            && pointer.is_some_and(|p| {
                rect.union(active.as_ref().map(|a| a.2).unwrap_or(bar))
                    .expand(6.0)
                    .contains(p)
            }))
    {
        return None;
    }
    let mut action = None;
    let toolbar = egui::Area::new(Id::new(("hover_actions", thread, &msg.id, msg.ts)))
        .order(egui::Order::Foreground)
        .fixed_pos(bar.left_top())
        .constrain_to(ui.clip_rect())
        .show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style())
                .inner_margin(3.0)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.menu_button("☺", |ui| {
                            ui.horizontal(|ui| {
                                for &(code, key, hint) in &REACTION_LIST {
                                    let response = if let Some(tex) = emoji.get(key) {
                                        ui.add(egui::ImageButton::new(
                                            egui::Image::new(tex)
                                                .fit_to_exact_size(Vec2::splat(24.0)),
                                        ))
                                    } else {
                                        ui.button(code)
                                    };
                                    if response.on_hover_text(hint).clicked() {
                                        action = Some(BubbleAction::React(code));
                                        ui.close_menu();
                                    }
                                }
                            });
                        })
                        .response
                        .on_hover_text("Thả cảm xúc");
                        if ui
                            .button("↩")
                            .on_hover_text(crate::i18n::tr("Trả lời"))
                            .clicked()
                        {
                            action = Some(BubbleAction::Reply);
                        }
                    });
                });
        });
    ui.ctx()
        .data_mut(|d| d.insert_temp(key, (msg.id.clone(), msg.ts, toolbar.response.rect)));
    action
}

#[cfg(test)]
mod hover_action_tests {
    use super::*;
    #[test]
    fn native_reaction_click_keeps_target_when_another_row_appears() {
        let ctx = egui::Context::default();
        let message = ChatMessage {
            id: "stable-target".into(),
            text: "hello".into(),
            ts: 100,
            ..Default::default()
        };
        let other = ChatMessage {
            id: "new-row".into(),
            text: "other".into(),
            ts: 200,
            ..Default::default()
        };
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 50.0), Vec2::new(600.0, 100.0));
        let mut point = rect.center();
        let mut result = None;
        for frame in 0..9 {
            let mut events = vec![egui::Event::PointerMoved(point)];
            if matches!(frame, 3 | 4 | 6 | 7) {
                events.push(egui::Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed: frame == 3 || frame == 6,
                    modifiers: Default::default(),
                });
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(800.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        if frame >= 2 {
                            // A neighboring row must not steal an already hovered toolbar.
                            let overlapping = rect.translate(Vec2::new(0.0, -30.0));
                            assert!(hover_message_actions(
                                ui,
                                overlapping,
                                "group_1",
                                &other,
                                &HashMap::new()
                            )
                            .is_none());
                        }
                        if let Some(action) =
                            hover_message_actions(ui, rect, "group_1", &message, &HashMap::new())
                        {
                            result = Some(action);
                        }
                    });
                },
            );
            if frame == 5 {
                point = output
                    .shapes
                    .iter()
                    .find_map(|shape| match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == "👍" => {
                            Some(text.pos + text.galley.size() * 0.5)
                        }
                        _ => None,
                    })
                    .expect("picker opens only after clicking the reaction button");
                assert!(result.is_none());
            }
            if frame == 1 {
                let (_, _, toolbar) = ctx
                    .data(|d| {
                        d.get_temp::<(String, i64, egui::Rect)>(Id::new((
                            "hover_message",
                            "group_1",
                        )))
                    })
                    .unwrap();
                point = toolbar.left_top() + Vec2::new(12.0, 12.0);
            }
        }
        assert!(matches!(result, Some(BubbleAction::React("👍"))));
    }
}
