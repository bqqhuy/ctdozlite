use super::*;

// ---------------------------------------------------------------------------
// Semantic Design System Tokens (UI/UX Pro Max — Minimalism & Swiss Style)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub(super) struct ThemePalette {
    pub(super) is_dark: bool,
    pub(super) bg_app: Color32,
    pub(super) bg_sidebar: Color32,
    pub(super) bg_chat_canvas: Color32,
    pub(super) bg_elevated: Color32,
    pub(super) bg_hover: Color32,
    pub(super) bg_selected: Color32,
    pub(super) bg_input: Color32,
    pub(super) border: Color32,
    pub(super) text_primary: Color32,
    pub(super) text_secondary: Color32,
    pub(super) text_muted: Color32,
    pub(super) primary: Color32,
    pub(super) primary_hover: Color32,
    pub(super) accent_online: Color32,
    pub(super) danger: Color32,
    pub(super) warning: Color32,
    pub(super) bubble_out_bg: Color32,
    pub(super) bubble_out_text: Color32,
    pub(super) bubble_out_meta: Color32,
    pub(super) bubble_in_bg: Color32,
    pub(super) bubble_in_border: Color32,
    pub(super) bubble_in_text: Color32,
    pub(super) bubble_in_meta: Color32,
}

impl ThemePalette {
    pub(super) fn apply_tools_theme(&mut self, value: &serde_json::Value) {
        // Legacy flat themes describe a single (dark) palette. Do not apply their
        // white foregrounds to light mode when no light palette was supplied.
        let section = match value.get(if self.is_dark { "dark" } else { "light" }) {
            Some(section) => section,
            None if self.is_dark && value.get("light").is_none() && value.get("dark").is_none() => {
                value
            }
            None => return,
        };
        for (name, target) in [
            ("bg_app", &mut self.bg_app),
            ("bg_sidebar", &mut self.bg_sidebar),
            ("bg_chat_canvas", &mut self.bg_chat_canvas),
            ("bg_elevated", &mut self.bg_elevated),
            ("bg_hover", &mut self.bg_hover),
            ("bg_selected", &mut self.bg_selected),
            ("bg_input", &mut self.bg_input),
            ("border", &mut self.border),
            ("text_primary", &mut self.text_primary),
            ("text_secondary", &mut self.text_secondary),
            ("text_muted", &mut self.text_muted),
            ("primary", &mut self.primary),
            ("primary_hover", &mut self.primary_hover),
            ("accent_online", &mut self.accent_online),
            ("danger", &mut self.danger),
            ("warning", &mut self.warning),
            ("bubble_out_bg", &mut self.bubble_out_bg),
            ("bubble_out_text", &mut self.bubble_out_text),
            ("bubble_out_meta", &mut self.bubble_out_meta),
            ("bubble_in_bg", &mut self.bubble_in_bg),
            ("bubble_in_border", &mut self.bubble_in_border),
            ("bubble_in_text", &mut self.bubble_in_text),
            ("bubble_in_meta", &mut self.bubble_in_meta),
        ] {
            if let Some(hex) = section
                .get(name)
                .and_then(|v| v.as_str())
                .and_then(|s| s.strip_prefix('#'))
                .filter(|s| s.len() == 6 && s.is_ascii())
            {
                if let Ok(value) = u32::from_str_radix(hex, 16) {
                    *target =
                        Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8);
                }
            }
        }
    }
    /// Keep custom foregrounds readable on every surface that shares them.
    pub(super) fn ensure_readable_text(&mut self) {
        let surfaces = [
            self.bg_app,
            self.bg_sidebar,
            self.bg_chat_canvas,
            self.bg_elevated,
            self.bg_hover,
            self.bg_selected,
            self.bg_input,
        ];
        for foreground in [
            &mut self.text_primary,
            &mut self.text_secondary,
            &mut self.text_muted,
        ] {
            *foreground = readable_foreground(*foreground, &surfaces);
        }
        self.bubble_in_text = readable_foreground(self.bubble_in_text, &[self.bubble_in_bg]);
        self.bubble_in_meta = readable_foreground(self.bubble_in_meta, &[self.bubble_in_bg]);
        self.bubble_out_text = readable_foreground(self.bubble_out_text, &[self.bubble_out_bg]);
        self.bubble_out_meta = readable_foreground(self.bubble_out_meta, &[self.bubble_out_bg]);
    }

    pub(super) fn dark() -> Self {
        // Authentic Zalo PC Dark Mode Design Tokens (par with Zalo PC assets & Olaz Theme.swift)
        Self {
            is_dark: true,
            bg_app: Color32::from_rgb(34, 38, 43), // #22262B Zalo Dark Base Window & Nav Rail
            bg_sidebar: Color32::from_rgb(42, 47, 53), // #2A2F35 Zalo Dark Elevated Sidebar
            bg_chat_canvas: Color32::from_rgb(30, 33, 37), // #1E2125 Zalo Dark Canvas
            bg_elevated: Color32::from_rgb(42, 47, 53), // #2A2F35 Elevated Card / Modal / Popover
            bg_hover: Color32::from_rgb(52, 58, 66), // #343A42 List Hover Surface
            bg_selected: Color32::from_rgb(26, 44, 74), // #1A2C4A Active Selection Row (Navy Tint)
            bg_input: Color32::from_rgb(30, 33, 37), // #1E2125 Input / Search Well
            border: Color32::from_rgb(50, 56, 64), // #323840 Subtle Panel Divider
            text_primary: Color32::from_rgb(241, 245, 249), // #F1F5F9 Bright Foreground
            text_secondary: Color32::from_rgb(180, 188, 200), // #B4BCC8 Secondary text
            text_muted: Color32::from_rgb(148, 163, 184), // #94A3B8 Muted text
            primary: Color32::from_rgb(0, 104, 255), // #0068FF Zalo Brand Blue
            primary_hover: Color32::from_rgb(0, 90, 224), // #005AE0 Zalo Blue Pressed
            accent_online: Color32::from_rgb(16, 185, 129), // #10B981 Emerald Green
            danger: Color32::from_rgb(239, 68, 68), // #EF4444 Red
            warning: Color32::from_rgb(245, 158, 11), // #F59E0B Amber
            bubble_out_bg: Color32::from_rgb(37, 55, 77), // #25374D Zalo Dark Slate-Navy Outgoing Bubble
            bubble_out_text: Color32::from_rgb(241, 245, 249), // #F1F5F9
            bubble_out_meta: Color32::from_rgb(148, 163, 184), // #94A3B8
            bubble_in_bg: Color32::from_rgb(42, 47, 53),  // #2A2F35 Zalo Dark Incoming Bubble
            bubble_in_border: Color32::from_rgb(50, 56, 64), // #323840 Gutter Border
            bubble_in_text: Color32::from_rgb(241, 245, 249), // #F1F5F9
            bubble_in_meta: Color32::from_rgb(148, 163, 184), // #94A3B8
        }
    }

    pub(super) fn light() -> Self {
        // Authentic Zalo PC Light Mode Design Tokens (par with Zalo PC assets & Olaz Theme.swift)
        Self {
            is_dark: false,
            bg_app: Color32::from_rgb(255, 255, 255), // #FFFFFF
            bg_sidebar: Color32::from_rgb(248, 250, 252), // #F8FAFC
            bg_chat_canvas: Color32::from_rgb(232, 238, 244), // #E8EEF4 Authentic Zalo PC cool blue-gray canvas
            bg_elevated: Color32::from_rgb(255, 255, 255),    // #FFFFFF
            bg_hover: Color32::from_rgb(241, 245, 249),       // #F1F5F9
            bg_selected: Color32::from_rgb(229, 241, 255),    // #E5F1FF Zalo active row
            bg_input: Color32::from_rgb(248, 250, 252),       // #F8FAFC
            border: Color32::from_rgb(226, 232, 240),         // #E2E8F0
            text_primary: Color32::from_rgb(8, 27, 58), // #081B3A Zalo Primary Ink (16.1:1 contrast)
            text_secondary: Color32::from_rgb(71, 85, 105), // #475569 (7.2:1 contrast)
            text_muted: Color32::from_rgb(100, 116, 139), // #64748B (4.6:1 contrast)
            primary: Color32::from_rgb(0, 104, 255),    // #0068FF Zalo Blue
            primary_hover: Color32::from_rgb(0, 90, 224), // #005AE0 Zalo Blue Pressed
            accent_online: Color32::from_rgb(5, 150, 105), // #059669 Emerald
            danger: Color32::from_rgb(220, 38, 38),     // #DC2626
            warning: Color32::from_rgb(146, 64, 14),    // #92400E
            bubble_out_bg: Color32::from_rgb(229, 239, 255), // #E5EFFF Zalo Desktop Light Outgoing
            bubble_out_text: Color32::from_rgb(8, 27, 58), // #081B3A Zalo Ink text on light blue
            bubble_out_meta: Color32::from_rgb(71, 92, 122),
            bubble_in_bg: Color32::WHITE,
            bubble_in_border: Color32::from_rgb(216, 224, 236),
            bubble_in_text: Color32::from_rgb(8, 27, 58), // #081B3A Zalo Ink text
            bubble_in_meta: Color32::from_rgb(100, 116, 139),
        }
    }
    pub(super) fn apply_to_ctx(&self, ctx: &egui::Context) {
        // egui stores separate light/dark styles. A late native system-theme
        // event must not switch to an unconfigured style behind our painted UI.
        let theme = if self.is_dark {
            egui::Theme::Dark
        } else {
            egui::Theme::Light
        };
        ctx.set_theme(theme);
        let mut style = (*ctx.style_of(theme)).clone();
        let mut visuals = if self.is_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        visuals.panel_fill = self.bg_app;
        visuals.window_fill = self.bg_elevated;
        visuals.extreme_bg_color = self.bg_input;
        visuals.faint_bg_color = self.bg_hover;
        visuals.override_text_color = Some(self.text_primary);
        visuals.hyperlink_color = self.primary_hover;
        visuals.warn_fg_color = self.warning;
        visuals.error_fg_color = self.danger;
        visuals.selection.bg_fill =
            self.primary
                .linear_multiply(if self.is_dark { 0.45 } else { 0.22 });
        visuals.selection.stroke = Stroke::new(1.5_f32, self.text_primary);

        let r = Rounding::same(8.0);
        visuals.widgets.noninteractive.bg_fill = self.bg_app;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.noninteractive.rounding = r;

        visuals.widgets.inactive.bg_fill = self.bg_elevated;
        visuals.widgets.inactive.weak_bg_fill = self.bg_elevated;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, self.border);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, self.text_primary);
        visuals.widgets.inactive.rounding = r;

        visuals.widgets.hovered.bg_fill = self.bg_hover;
        visuals.widgets.hovered.weak_bg_fill = self.bg_hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.2_f32, self.primary);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.2_f32, self.text_primary);
        visuals.widgets.hovered.rounding = r;

        visuals.widgets.active.bg_fill = self.primary;
        visuals.widgets.active.weak_bg_fill = self.primary;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, self.primary);
        visuals.widgets.active.fg_stroke = Stroke::new(
            1.5_f32,
            readable_foreground(Color32::WHITE, &[self.primary]),
        );
        visuals.widgets.active.rounding = r;

        style.spacing.item_spacing = Vec2::new(8.0, 6.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        style.spacing.scroll = egui::style::ScrollStyle::solid();
        style.spacing.scroll.bar_width = 6.0;
        style.visuals = visuals;
        ctx.set_style_of(theme, style);
    }
}

fn contrast_ratio(a: Color32, b: Color32) -> f32 {
    fn luminance(c: Color32) -> f32 {
        let linear = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    }
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn readable_foreground(requested: Color32, backgrounds: &[Color32]) -> Color32 {
    let minimum = |fg| {
        backgrounds
            .iter()
            .map(|bg| contrast_ratio(fg, *bg))
            .fold(f32::INFINITY, f32::min)
    };
    if requested.a() == 255 && minimum(requested) >= 4.5 {
        return requested;
    }
    let dark = Color32::from_rgb(15, 23, 42);
    let light = Color32::from_rgb(241, 245, 249);
    if minimum(dark) >= minimum(light) {
        dark
    } else {
        light
    }
}

// ---------------------------------------------------------------------------
// Zero-allocation Vector Icon Painter Primitives (Replaces Emoji Icons)
// ---------------------------------------------------------------------------

pub(super) fn avatar_color_for(name: &str, is_group: bool) -> Color32 {
    const PALETTE: &[Color32] = &[
        Color32::from_rgb(64, 120, 192), // Atom Blue (#4078C0)
        Color32::from_rgb(56, 148, 160), // Atom Cyan (#3894A0)
        Color32::from_rgb(152, 88, 178), // Atom Purple (#9858B2)
        Color32::from_rgb(190, 80, 70),  // Atom Red (#BE5046)
        Color32::from_rgb(184, 118, 62), // Atom Orange (#B8763E)
        Color32::from_rgb(98, 148, 72),  // Atom Green (#629448)
        Color32::from_rgb(82, 116, 196), // Atom Indigo
        Color32::from_rgb(72, 138, 182), // Atom Cerulean
    ];
    let mut hash: usize = if is_group { 3 } else { 0 };
    for b in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as usize);
    }
    PALETTE[hash % PALETTE.len()]
}

pub(super) fn truncate_chars<'a>(s: &'a str, max_chars: usize) -> std::borrow::Cow<'a, str> {
    use unicode_segmentation::UnicodeSegmentation;
    if let Some((byte_idx, _)) = s.grapheme_indices(true).nth(max_chars) {
        std::borrow::Cow::Owned(format!("{}…", &s[..byte_idx]))
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// Extract up to 2 uppercase alphanumeric initials from a display name (e.g. `"Nguyễn An"` -> `"NA"`, `"Hoang"` -> `"H"`).
pub(super) fn avatar_initials(name: &str) -> String {
    let clean_words: Vec<&str> = name
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|w| !w.is_empty())
        .collect();
    if clean_words.len() >= 2 {
        let c1 = clean_words[0].chars().next().unwrap_or('Z').to_uppercase();
        let c2 = clean_words[clean_words.len() - 1]
            .chars()
            .next()
            .unwrap_or('U')
            .to_uppercase();
        format!("{c1}{c2}")
    } else if let Some(first) = clean_words.first() {
        first
            .chars()
            .next()
            .unwrap_or('Z')
            .to_uppercase()
            .to_string()
    } else {
        "Z".to_string()
    }
}

/// Fold Vietnamese text to lowercase ASCII for diacritic-insensitive search
/// (e.g. `"Nguyễn Văn An"` -> `"nguyen van an"`).
pub(super) fn fold_vn_search(s: &str) -> String {
    crate::search::fold(s)
}

/// Check whether `haystack` matches `query_lower` (or `query_folded` without Vietnamese accents).
pub(super) fn matches_vn_query(haystack: &str, query_lower: &str, query_folded: &str) -> bool {
    if query_lower.is_empty() {
        return true;
    }
    if haystack.to_lowercase().contains(query_lower) {
        return true;
    }
    !query_folded.is_empty() && fold_vn_search(haystack).contains(query_folded)
}

/// Centre-crop the texture to a square, then draw it inside a circle so non-square
/// avatars are neither stretched nor shown with corners.
fn paint_circular_image(painter: &egui::Painter, center: Pos2, radius: f32, tex: &TextureHandle) {
    let (uv_min, uv_max) = square_crop(tex.size());
    let mut mesh = egui::Mesh::with_texture(tex.id());
    let steps = 40u32;
    mesh.vertices.push(egui::epaint::Vertex {
        pos: center,
        uv: Pos2::new((uv_min.x + uv_max.x) * 0.5, (uv_min.y + uv_max.y) * 0.5),
        color: Color32::WHITE,
    });
    for i in 0..=steps {
        let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
        let (sin, cos) = angle.sin_cos();
        mesh.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(center.x + radius * cos, center.y + radius * sin),
            uv: Pos2::new(
                uv_min.x + (cos * 0.5 + 0.5) * (uv_max.x - uv_min.x),
                uv_min.y + (sin * 0.5 + 0.5) * (uv_max.y - uv_min.y),
            ),
            color: Color32::WHITE,
        });
        if i > 0 {
            mesh.indices.extend_from_slice(&[0, i, i + 1]);
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}
/// UV rectangle of the centred square inside a `[width, height]` texture.
fn square_crop([w, h]: [usize; 2]) -> (Pos2, Pos2) {
    if w == h || w == 0 || h == 0 {
        (Pos2::ZERO, Pos2::new(1.0, 1.0))
    } else if w > h {
        let crop = (w - h) as f32 / (2.0 * w as f32);
        (Pos2::new(crop, 0.0), Pos2::new(1.0 - crop, 1.0))
    } else {
        let crop = (h - w) as f32 / (2.0 * h as f32);
        (Pos2::new(0.0, crop), Pos2::new(1.0, 1.0 - crop))
    }
}

// Rendering arguments describe one immediate-mode paint operation.
#[allow(clippy::too_many_arguments)]
fn paint_single_circle_avatar(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    name: &str,
    is_group: bool,
    avatar_url: Option<&str>,
    textures: &HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
) {
    if let Some(url) = avatar_url.map(str::trim).filter(|u| {
        !u.is_empty()
            && !u.ends_with("/default")
            && (u.starts_with("http://") || u.starts_with("https://"))
    }) {
        let file_or_base = crate::chat::avatar_file_key(Some(url))
            .unwrap_or_else(|| url.split('?').next().unwrap_or(url).to_string());
        let key = format!("ava_{}", crate::crypto::md5_hex(&file_or_base));
        if let Some(tex) = textures.get(&key) {
            paint_circular_image(painter, center, radius, tex);
            painter.circle_stroke(
                center,
                radius,
                Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 34)),
            );
            return;
        }
        if painter
            .clip_rect()
            .expand(64.0)
            .intersects(Rect::from_center_size(center, Vec2::splat(radius * 2.0)))
            && loading.insert(key.clone())
        {
            if let Some(tx) = WORKER_TX.get() {
                let _ = tx.send(WorkerCmd::FetchStickerImage {
                    key,
                    url: url.to_string(),
                });
            }
        }
    }

    let av_col = avatar_color_for(name, is_group);
    painter.circle_filled(center, radius, av_col);
    let initial = if radius >= 15.5 {
        avatar_initials(name)
    } else {
        name.trim_matches(|c: char| !c.is_alphanumeric())
            .chars()
            .next()
            .unwrap_or('Z')
            .to_uppercase()
            .to_string()
    };
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        initial,
        FontId::proportional((radius * 0.76).max(8.5)),
        Color32::WHITE,
    );
}

/// Paint a circular user or group avatar:
/// - For 1-1 contacts or groups with a custom uploaded group photo (`http(s)://...`), renders the circular image.
/// - For groups without a single custom photo (`multi:...` or `None`), renders Zalo PC's `zavatar-multi`
///   composite layout (3-member triangle or 2x2 member grid with optional member count badge).
// Rendering arguments describe one immediate-mode paint operation.
#[allow(clippy::too_many_arguments)]
pub(super) fn paint_user_avatar(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    name: &str,
    is_group: bool,
    avatar_url: Option<&str>,
    textures: &HashMap<String, TextureHandle>,
    loading: &mut HashSet<String>,
) {
    let raw_ava = avatar_url.map(str::trim).unwrap_or("");
    let has_single_custom_photo = !raw_ava.is_empty()
        && !raw_ava.starts_with("multi:")
        && !raw_ava.ends_with("/default")
        && (raw_ava.starts_with("http://") || raw_ava.starts_with("https://"));

    if !is_group || has_single_custom_photo {
        paint_single_circle_avatar(
            painter, center, radius, name, is_group, avatar_url, textures, loading,
        );
        return;
    }

    // Parse composite group member slots (`multi:<total>;<name1>|<url1>;<name2>|<url2>;...`)
    // or derive member slots from comma-separated group name (`"Đỗ Thành, Đtc, Hoang"`).
    let mut slots: Vec<(String, String)> = Vec::new();
    let mut total_members: usize = 0;
    if let Some(rest) = raw_ava.strip_prefix("multi:") {
        for (idx, part) in rest.split(';').enumerate() {
            if idx == 0 && !part.contains('|') {
                total_members = part.trim().parse::<usize>().unwrap_or(0);
                continue;
            }
            if let Some((m_name, m_url)) = part.split_once('|') {
                slots.push((m_name.trim().to_string(), m_url.trim().to_string()));
            } else if !part.trim().is_empty() {
                slots.push((part.trim().to_string(), String::new()));
            }
        }
    }
    if slots.is_empty() {
        let comma_parts: Vec<&str> = name
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if comma_parts.len() >= 2 {
            for p in comma_parts.iter().take(4) {
                slots.push(((*p).to_string(), String::new()));
            }
            total_members = comma_parts.len();
        } else {
            let words: Vec<&str> = name.split_whitespace().filter(|w| !w.is_empty()).collect();
            if words.len() >= 3 {
                for w in words.iter().take(3) {
                    slots.push(((*w).to_string(), String::new()));
                }
            } else {
                slots.push((name.to_string(), String::new()));
                slots.push(("TV".to_string(), String::new()));
                slots.push(("N".to_string(), String::new()));
            }
            total_members = slots.len();
        }
    }
    if total_members < slots.len() {
        total_members = slots.len();
    }

    let border_col = Color32::from_rgb(24, 27, 34);
    let n_slots = slots.len();
    if n_slots == 2 {
        let sub_r = (radius * 0.61).max(6.0);
        let p1 = Pos2::new(center.x + radius - sub_r, center.y - radius + sub_r);
        let p0 = Pos2::new(center.x - radius + sub_r, center.y + radius - sub_r);
        for &(pos, idx) in &[(p1, 1usize), (p0, 0usize)] {
            painter.circle_filled(pos, sub_r + 1.2, border_col);
            let (ref m_name, ref m_url) = slots[idx];
            paint_single_circle_avatar(
                painter,
                pos,
                sub_r,
                m_name,
                false,
                if m_url.is_empty() {
                    None
                } else {
                    Some(m_url.as_str())
                },
                textures,
                loading,
            );
        }
    } else if n_slots == 3 && total_members <= 3 {
        // Zalo PC `zavatar-multi.zavatar-3`: 1 top-center (z=4), 1 bottom-left (z=3), 1 bottom-right (z=2)
        let sub_r = (radius * 0.56).max(5.5);
        let p_top = Pos2::new(center.x, center.y - radius + sub_r);
        let p_bl = Pos2::new(
            center.x - radius + sub_r + 0.5,
            center.y + radius - sub_r - 0.5,
        );
        let p_br = Pos2::new(
            center.x + radius - sub_r - 0.5,
            center.y + radius - sub_r - 0.5,
        );
        for &(pos, idx) in &[(p_br, 2usize), (p_bl, 1usize), (p_top, 0usize)] {
            painter.circle_filled(pos, sub_r + 1.2, border_col);
            let (ref m_name, ref m_url) = slots[idx];
            paint_single_circle_avatar(
                painter,
                pos,
                sub_r,
                m_name,
                false,
                if m_url.is_empty() {
                    None
                } else {
                    Some(m_url.as_str())
                },
                textures,
                loading,
            );
        }
    } else {
        // Zalo PC `zavatar-multi.zavatar-4`: 2x2 grid (with member count badge in bottom-right if total_members >= 4)
        let sub_r = (radius * 0.52).max(5.0);
        let p_tl = Pos2::new(center.x - radius + sub_r, center.y - radius + sub_r);
        let p_tr = Pos2::new(center.x + radius - sub_r, center.y - radius + sub_r);
        let p_bl = Pos2::new(center.x - radius + sub_r, center.y + radius - sub_r);
        let p_br = Pos2::new(center.x + radius - sub_r, center.y + radius - sub_r);
        let show_counter = total_members > 4 || n_slots < 4;
        if show_counter {
            painter.circle_filled(p_br, sub_r + 1.2, border_col);
            painter.circle_filled(p_br, sub_r, Color32::from_rgb(56, 65, 82));
            let cnt_txt = if total_members > 99 {
                "99+".to_string()
            } else {
                total_members.to_string()
            };
            painter.text(
                p_br,
                egui::Align2::CENTER_CENTER,
                cnt_txt,
                FontId::proportional((sub_r * 0.76).max(8.0)),
                Color32::from_rgb(218, 226, 240),
            );
        } else if let Some((ref m_name, ref m_url)) = slots.get(3) {
            painter.circle_filled(p_br, sub_r + 1.2, border_col);
            paint_single_circle_avatar(
                painter,
                p_br,
                sub_r,
                m_name,
                false,
                if m_url.is_empty() {
                    None
                } else {
                    Some(m_url.as_str())
                },
                textures,
                loading,
            );
        }
        for &(pos, idx) in &[(p_bl, 2usize), (p_tr, 1usize), (p_tl, 0usize)] {
            if let Some((ref m_name, ref m_url)) = slots.get(idx) {
                painter.circle_filled(pos, sub_r + 1.2, border_col);
                paint_single_circle_avatar(
                    painter,
                    pos,
                    sub_r,
                    m_name,
                    false,
                    if m_url.is_empty() {
                        None
                    } else {
                        Some(m_url.as_str())
                    },
                    textures,
                    loading,
                );
            }
        }
    }
}

pub mod lucide {
    pub const SEARCH: &str = "\u{e151}";
    pub const REFRESH: &str = "\u{e145}";
    pub const SMILE: &str = "\u{e164}";
    pub const SEND: &str = "\u{e4f2}";
    pub const SUN: &str = "\u{e178}";
    pub const MOON: &str = "\u{e11e}";
    pub const PHONE: &str = "\u{e133}";
    pub const PHONE_CALL: &str = "\u{e134}";
    pub const PHONE_OFF: &str = "\u{e138}";
    pub const VIDEO: &str = "\u{e1a5}";
    pub const VIDEO_OFF: &str = "\u{e1a6}";
    pub const CAMERA: &str = "\u{e062}";
    pub const IMAGE: &str = "\u{e0f6}";
    pub const PAPERCLIP: &str = "\u{e12d}";
    pub const CHECK: &str = "\u{e06c}";
    pub const CHECK_CHECK: &str = "\u{e38e}";
    pub const MESSAGE_SQUARE: &str = "\u{e117}";
    pub const USERS: &str = "\u{e1a4}";
    pub const USER_PLUS: &str = "\u{e1a2}";
    pub const CLOUD: &str = "\u{e088}";
    pub const SETTINGS: &str = "\u{e154}";
    pub const PANEL_RIGHT: &str = "\u{e431}";
    pub const PIN: &str = "\u{e259}";
    pub const COPY: &str = "\u{e09e}";
    pub const STAR: &str = "\u{e176}";
    pub const GLOBE: &str = "\u{e0e8}";
    pub const LINK: &str = "\u{e102}";
    pub const EXTERNAL_LINK: &str = "\u{e0b9}";
    pub const X: &str = "\u{e1b2}";
    pub const PLUS: &str = "\u{e13d}";
    pub const MINUS: &str = "\u{e11c}";
    pub const STORE: &str = "\u{e3e4}";
    pub const FLAME: &str = "\u{e0d2}";
    pub const SPARKLES: &str = "\u{e412}";
    pub const BAN: &str = "\u{e051}";
    pub const MIC: &str = "\u{e118}";
    pub const MIC_OFF: &str = "\u{e119}";
    pub const LOG_OUT: &str = "\u{e10e}";
    pub const FILE_TEXT: &str = "\u{e0cc}";
    pub const FILM: &str = "\u{e0d0}";
    pub const SHIELD_CHECK: &str = "\u{e15c}";
    pub const LOCK: &str = "\u{e10b}";
    pub const BELL: &str = "\u{e059}";
    pub const PAINTBRUSH: &str = "\u{e2e7}";
    pub const SLIDERS: &str = "\u{e29a}";
    pub const FOLDER_OPEN: &str = "\u{e247}";
    pub const CHEVRON_RIGHT: &str = "\u{e06f}";
    pub const CHEVRON_DOWN: &str = "\u{e06d}";
    pub const USER: &str = "\u{e19f}";
}

pub(super) fn draw_lucide_icon(
    painter: &egui::Painter,
    center: Pos2,
    icon: &str,
    size: f32,
    color: Color32,
) {
    painter.text(
        center,
        egui::Align2::CENTER_CENTER,
        icon,
        FontId::proportional(size),
        color,
    );
}

pub(super) fn draw_icon_search(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SEARCH, size, color);
}

pub(super) fn draw_icon_refresh(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::REFRESH, size, color);
}

pub(super) fn draw_icon_sticker(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SMILE, size, color);
}

pub(super) fn draw_icon_send(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SEND, size, color);
}

pub(super) fn draw_icon_phone(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::PHONE, size, color);
}

pub(super) fn draw_icon_video(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::VIDEO, size, color);
}

pub(super) fn draw_icon_image(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::IMAGE, size, color);
}

pub(super) fn draw_icon_attachment(
    painter: &egui::Painter,
    center: Pos2,
    size: f32,
    color: Color32,
) {
    draw_lucide_icon(painter, center, lucide::PAPERCLIP, size, color);
}

pub(super) fn draw_icon_chat_nav(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::MESSAGE_SQUARE, size, color);
}

pub(super) fn draw_icon_contacts(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::USERS, size, color);
}

pub(super) fn draw_icon_cloud(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::CLOUD, size, color);
}

pub(super) fn draw_icon_settings(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::SETTINGS, size, color);
}

pub(super) fn draw_icon_drawer(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::PANEL_RIGHT, size, color);
}

pub(super) fn draw_icon_pin(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::PIN, size, color);
}

pub(super) fn draw_icon_copy(painter: &egui::Painter, center: Pos2, size: f32, color: Color32) {
    draw_lucide_icon(painter, center, lucide::COPY, size, color);
}

pub(super) fn draw_icon_user_plus(
    painter: &egui::Painter,
    center: Pos2,
    size: f32,
    color: Color32,
) {
    draw_lucide_icon(painter, center, lucide::USER_PLUS, size, color);
}

pub const REACTION_LIST: [(&str, &str, &str); 6] = [
    ("👍", "like", "Thích"),
    ("❤️", "heart", "Yêu thích"),
    ("😆", "smile", "Haha"),
    ("😮", "surprised", "Wow"),
    ("😢", "cry", "Buồn"),
    ("😠", "rage", "Phẫn nộ"),
];

pub fn emoji_to_tex_key(emoji: &str) -> &'static str {
    match emoji {
        "👍" | "/-strong" => "like",
        "❤️" | "❤" | "/-heart" => "heart",
        "😆" | ":>" => "smile",
        "😮" | ":o" => "surprised",
        "😢" | ":-(( " | ":-((" => "cry",
        "😠" | ":-h" => "rage",
        _ => "like",
    }
}

pub(super) const APP_ICON_PNG: &[u8] = include_bytes!("../assets/app_icon.png");

/// Decode the embedded 256x256 RGBA application icon for the OS window/taskbar (`ViewportBuilder::with_icon`).
pub fn load_app_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

pub(super) fn load_app_logo_texture(ctx: &egui::Context) -> Option<TextureHandle> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let color_img =
        egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], img.as_raw());
    Some(ctx.load_texture("zalo_app_logo", color_img, TextureOptions::LINEAR))
}

pub(super) fn load_default_emoji_textures(ctx: &egui::Context) -> HashMap<String, TextureHandle> {
    let raw: [(&str, &[u8]); 6] = [
        ("like", include_bytes!("../assets/emojis/like.png")),
        ("heart", include_bytes!("../assets/emojis/heart.png")),
        ("smile", include_bytes!("../assets/emojis/smile.png")),
        (
            "surprised",
            include_bytes!("../assets/emojis/surprised.png"),
        ),
        ("cry", include_bytes!("../assets/emojis/cry.png")),
        ("rage", include_bytes!("../assets/emojis/rage.png")),
    ];
    let mut map = HashMap::new();
    for (name, bytes) in raw {
        if let Ok(img) = image::load_from_memory(bytes) {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let color_img =
                egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], rgba.as_raw());
            let tex = ctx.load_texture(
                format!("zalo_emoji_{name}"),
                color_img,
                TextureOptions::LINEAR,
            );
            map.insert(name.to_string(), tex);
        }
    }
    map
}

/// Render an iOS/Zalo PC style rounded Toggle Switch in pure egui.
pub(super) fn render_toggle_switch(
    ui: &mut egui::Ui,
    value: &mut bool,
    pal: &ThemePalette,
) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(38.0, 20.0), Sense::click());
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    resp = resp.on_hover_cursor(CursorIcon::PointingHand);

    let bg = if *value {
        pal.primary
    } else if pal.is_dark {
        Color32::from_rgb(58, 63, 75)
    } else {
        Color32::from_rgb(203, 213, 225)
    };
    ui.painter().rect_filled(rect, Rounding::same(10.0), bg);

    let knob_radius = 7.5_f32;
    let knob_x = if *value {
        rect.right() - 10.0
    } else {
        rect.left() + 10.0
    };
    ui.painter().circle_filled(
        Pos2::new(knob_x, rect.center().y),
        knob_radius,
        Color32::WHITE,
    );
    resp
}

static CJK_LOADED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11FF | 0x3040..=0x30FF | 0x3130..=0x318F | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF)
}

/// Load the CJK fallback the first time Han / Kana / Hangul text shows up. msyh.ttc alone is
/// ~20 MB held in memory for the whole run, which a Vietnamese-only user never needs.
pub(super) fn ensure_cjk_font<'a>(ctx: &egui::Context, mut texts: impl Iterator<Item = &'a str>) {
    use std::sync::atomic::Ordering;
    if CJK_LOADED.load(Ordering::Relaxed) || !texts.any(|t| t.chars().any(is_cjk)) {
        return;
    }
    CJK_LOADED.store(true, Ordering::Relaxed);
    build_fonts(ctx, true);
}

pub(super) fn setup_custom_fonts(ctx: &egui::Context) {
    build_fonts(ctx, CJK_LOADED.load(std::sync::atomic::Ordering::Relaxed));
}

fn build_fonts(ctx: &egui::Context, with_cjk: bool) {
    let mut fonts = egui::FontDefinitions::default();

    // 1. Embed official Lucide Icon Font (vector glyphs for all UI icons)
    const LUCIDE_BYTES: &[u8] = include_bytes!("../assets/fonts/lucide.ttf");
    fonts.font_data.insert(
        "lucide_font".to_owned(),
        egui::FontData::from_static(LUCIDE_BYTES),
    );

    // Bundled OFL font guarantees Vietnamese coverage without depending on the host.
    fonts.font_data.insert(
        "bundled_sans".into(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/BeVietnamPro-Regular.ttf")),
    );
    // Read installed Windows fonts; do not redistribute proprietary font files.
    let font_dir = std::env::var_os("WINDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Windows"))
        .join("Fonts");
    for name in ["segoeui.ttf", "arial.ttf", "tahoma.ttf"] {
        if let Ok(data) = std::fs::read(font_dir.join(name)) {
            fonts
                .font_data
                .insert("vietnamese_font".into(), egui::FontData::from_owned(data));
            break;
        }
    }
    if with_cjk {
        for name in ["msyh.ttc", "simsun.ttc", "meiryo.ttc", "malgun.ttf"] {
            if let Ok(data) = std::fs::read(font_dir.join(name)) {
                fonts
                    .font_data
                    .insert("cjk_font".into(), egui::FontData::from_owned(data));
                break;
            }
        }
    }

    // Fallback order: bundled sans guarantees Vietnamese coverage, CJK supplies Han/Kana/Hangul, Lucide supplies icon glyphs
    let prop = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    prop.insert(0, "bundled_sans".to_owned());
    if fonts.font_data.contains_key("vietnamese_font") {
        prop.insert(1, "vietnamese_font".to_owned());
    }
    if fonts.font_data.contains_key("cjk_font") {
        prop.push("cjk_font".to_owned());
    }
    prop.push("lucide_font".to_owned());

    let mono = fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default();
    mono.push("bundled_sans".to_owned());
    if fonts.font_data.contains_key("vietnamese_font") {
        mono.push("vietnamese_font".to_owned());
    }
    if fonts.font_data.contains_key("cjk_font") {
        mono.push("cjk_font".to_owned());
    }
    mono.push("lucide_font".to_owned());

    ctx.data_mut(|data| data.insert_temp(egui::Id::new("available-ui-fonts"), fonts.clone()));
    let family = ctx
        .data(|data| data.get_temp::<String>(egui::Id::new("preferred-ui-font")))
        .unwrap_or_default();
    select_font(ctx, &family);
}

#[cfg(test)]
mod theme_tests {
    use super::*;
    #[test]
    fn selected_palette_survives_opposite_system_theme_arriving() {
        for (palette, system_theme) in [
            (ThemePalette::dark(), egui::Theme::Light),
            (ThemePalette::light(), egui::Theme::Dark),
        ] {
            let ctx = egui::Context::default();
            palette.apply_to_ctx(&ctx);
            let _ = ctx.run(
                egui::RawInput {
                    system_theme: Some(system_theme),
                    ..Default::default()
                },
                |ctx| {
                    assert_eq!(
                        ctx.style().visuals.override_text_color,
                        Some(palette.text_primary)
                    );
                    assert_eq!(
                        ctx.style().visuals.widgets.inactive.bg_fill,
                        palette.bg_elevated
                    );
                },
            );
        }
    }
}

#[cfg(test)]
mod avatar_crop_tests {
    use super::*;
    #[test]
    fn crop_keeps_the_centred_square() {
        assert_eq!(square_crop([100, 100]), (Pos2::ZERO, Pos2::new(1.0, 1.0)));
        let (min, max) = square_crop([200, 100]);
        assert!((min.x - 0.25).abs() < 1e-6 && (max.x - 0.75).abs() < 1e-6);
        assert_eq!((min.y, max.y), (0.0, 1.0));
        let (min, max) = square_crop([100, 400]);
        assert!((min.y - 0.375).abs() < 1e-6 && (max.y - 0.625).abs() < 1e-6);
    }
}

#[cfg(test)]
mod custom_theme_tests {
    use super::*;
    #[test]
    fn dark_only_custom_colors_do_not_leak_into_light_mode() {
        let flat = serde_json::json!({"text_primary":"#ffffff","bg_app":"#111111"});
        let mut light = ThemePalette::light();
        let expected = light.text_primary;
        light.apply_tools_theme(&flat);
        assert_eq!(light.text_primary, expected);
        let mut dark = ThemePalette::dark();
        dark.apply_tools_theme(&flat);
        assert_eq!(dark.text_primary, Color32::WHITE);
        let mut dark = ThemePalette::dark();
        let expected = dark.text_primary;
        dark.apply_tools_theme(&serde_json::json!({"light":{"text_primary":"#112233"}}));
        assert_eq!(dark.text_primary, expected);
        light.apply_tools_theme(&serde_json::json!({"light":{"text_primary":"#112233"}}));
        assert_eq!(light.text_primary, Color32::from_rgb(17, 34, 51));
    }
}

#[cfg(test)]
mod contrast_tests {
    use super::*;

    #[test]
    fn light_custom_white_foregrounds_remain_readable() {
        let mut palette = ThemePalette::light();
        palette.apply_tools_theme(&serde_json::json!({"light":{
            "text_primary":"#ffffff", "text_secondary":"#ffffff",
            "text_muted":"#ffffff", "bubble_out_text":"#ffffff"
        }}));
        palette.ensure_readable_text();
        for background in [
            palette.bg_app,
            palette.bg_sidebar,
            palette.bg_elevated,
            palette.bg_input,
            palette.bg_hover,
            palette.bg_selected,
        ] {
            assert!(contrast_ratio(palette.text_primary, background) >= 4.5);
            assert!(contrast_ratio(palette.text_secondary, background) >= 4.5);
            assert!(contrast_ratio(palette.text_muted, background) >= 4.5);
        }
        assert!(contrast_ratio(palette.bubble_out_text, palette.bubble_out_bg) >= 4.5);
    }
}

#[cfg(test)]
mod name_truncation_tests {
    use super::*;
    #[test]
    fn truncation_preserves_combining_accents_and_joined_emoji() {
        assert_eq!(truncate_chars("A👨‍👩‍👧‍👦B", 2), "A👨‍👩‍👧‍👦…");
        assert_eq!(truncate_chars("a\u{0301}b", 1), "a\u{0301}…");
        assert_eq!(truncate_chars("🇻🇳", 1), "🇻🇳");
    }
}

/// Persist only bundled/installed choices; configuration never loads arbitrary font paths.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(default)]
pub(super) struct AppearanceSettings {
    pub font: String,
    pub zoom: f32,
    pub custom_colors: bool,
    pub primary: [u8; 3],
    pub accent: [u8; 3],
    pub bubble: [u8; 3],
}
impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            font: "bundled".into(),
            zoom: 1.0,
            custom_colors: false,
            primary: [0, 104, 255],
            accent: [55, 156, 90],
            bubble: [220, 237, 255],
        }
    }
}
impl AppearanceSettings {
    pub(super) fn apply_context(&self, ctx: &egui::Context) {
        let zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.8, 1.5)
        } else {
            1.0
        };
        ctx.set_zoom_factor(zoom);
        select_font(ctx, &self.font);
    }
    pub(super) fn apply_palette(&self, palette: &mut ThemePalette) {
        if self.custom_colors {
            palette.primary = Color32::from_rgb(self.primary[0], self.primary[1], self.primary[2]);
            // Existing primary-filled buttons use white text. Keep their contrast while
            // choosing a separate readable foreground for links and selected labels.
            while contrast_ratio(Color32::WHITE, palette.primary) < 4.5 {
                palette.primary = Color32::from_rgb(
                    (f32::from(palette.primary.r()) * 0.9) as u8,
                    (f32::from(palette.primary.g()) * 0.9) as u8,
                    (f32::from(palette.primary.b()) * 0.9) as u8,
                );
            }
            palette.primary_hover = readable_foreground(
                Color32::from_rgb(self.primary[0], self.primary[1], self.primary[2]),
                &[palette.bg_app, palette.bg_sidebar, palette.bg_elevated],
            );
            palette.accent_online = readable_foreground(
                Color32::from_rgb(self.accent[0], self.accent[1], self.accent[2]),
                &[palette.bg_app, palette.bg_sidebar],
            );
            palette.bubble_out_bg =
                Color32::from_rgb(self.bubble[0], self.bubble[1], self.bubble[2]);
        }
    }
}

fn select_font(ctx: &egui::Context, choice: &str) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("preferred-ui-font"), choice.to_owned()));
    let Some(mut fonts) = ctx
        .data(|data| data.get_temp::<egui::FontDefinitions>(egui::Id::new("available-ui-fonts")))
    else {
        return;
    };
    let primary = match choice {
        "system" if fonts.font_data.contains_key("vietnamese_font") => "vietnamese_font",
        _ => "bundled_sans",
    };
    let family = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    family.retain(|name| name != primary);
    family.insert(0, primary.to_owned());
    ctx.set_fonts(fonts);
}

impl ZaloGuiApp {
    pub(super) fn appearance_controls(&mut self, ui: &mut egui::Ui) {
        let before = self.settings.appearance.clone();
        let settings = &mut self.settings.appearance;
        ui.separator();
        ui.label("Phông chữ và kích thước");
        egui::ComboBox::from_id_salt("ui-font")
            .selected_text(if settings.font == "system" {
                "Phông hệ thống (có dự phòng)"
            } else {
                "Be Vietnam Pro"
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut settings.font, "bundled".into(), "Be Vietnam Pro");
                ui.selectable_value(
                    &mut settings.font,
                    "system".into(),
                    "Phông hệ thống (có dự phòng)",
                );
            });
        ui.add(egui::Slider::new(&mut settings.zoom, 0.8..=1.5).text("Tỷ lệ giao diện"));
        ui.horizontal_wrapped(|ui| {
            for (label, zoom) in [("Nhỏ", 0.9), ("Chuẩn", 1.0), ("Lớn", 1.2)] {
                if ui.button(label).clicked() {
                    settings.zoom = zoom;
                }
            }
        });
        ui.checkbox(&mut settings.custom_colors, "Tùy chỉnh màu");
        if settings.custom_colors {
            ui.horizontal_wrapped(|ui| {
                for (name, primary, accent, bubble) in [
                    ("Xanh dương", [0, 104, 255], [0, 130, 110], [220, 238, 255]),
                    ("Xanh lá", [0, 120, 85], [30, 110, 160], [218, 242, 228]),
                    ("Tím", [110, 70, 190], [0, 125, 130], [238, 226, 250]),
                ] {
                    if ui.button(name).clicked() {
                        settings.primary = primary;
                        settings.accent = accent;
                        settings.bubble = bubble;
                    }
                }
            });
            for (label, rgb) in [
                ("Màu chính", &mut settings.primary),
                ("Trạng thái", &mut settings.accent),
                ("Bong bóng gửi", &mut settings.bubble),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    ui.color_edit_button_srgb(rgb);
                });
            }
        }
        if ui.button("Khôi phục giao diện mặc định").clicked() {
            *settings = Default::default();
        }
        if *settings != before {
            self.theme_dirty = true;
            // Save once after interaction rather than for every frame of a color drag.
            ui.ctx().request_repaint();
        }
        if ui.button("Lưu tùy chỉnh giao diện").clicked() {
            self.save_settings();
        }
        let palette = self.palette();
        egui::Frame::none()
            .fill(palette.bubble_out_bg)
            .inner_margin(8.0)
            .rounding(8.0)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Xem trước: Tiếng Việt có dấu • 😀")
                        .color(palette.bubble_out_text),
                );
                ui.label(RichText::new("12:30 • Đã gửi").color(palette.bubble_out_meta));
            });
    }
}

#[cfg(test)]
mod customization_tests {
    use super::*;
    #[test]
    fn custom_bubble_keeps_text_readable_in_both_modes_and_settings_roundtrip() {
        let settings = AppearanceSettings {
            custom_colors: true,
            bubble: [255, 255, 255],
            ..Default::default()
        };
        let parsed: AppearanceSettings =
            serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(settings, parsed);
        for mut palette in [ThemePalette::light(), ThemePalette::dark()] {
            settings.apply_palette(&mut palette);
            palette.ensure_readable_text();
            assert!(contrast_ratio(Color32::WHITE, palette.primary) >= 4.5);
            assert!(contrast_ratio(palette.bubble_out_text, palette.bubble_out_bg) >= 4.5);
            assert!(contrast_ratio(palette.bubble_out_meta, palette.bubble_out_bg) >= 4.5);
        }
    }
    #[test]
    fn unknown_font_and_invalid_scale_fall_back_without_reading_paths() {
        let ctx = egui::Context::default();
        setup_custom_fonts(&ctx);
        AppearanceSettings {
            font: "../../untrusted.ttf".into(),
            zoom: f32::NAN,
            ..Default::default()
        }
        .apply_context(&ctx);
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.label("Tiếng Việt 😀");
            });
        });
        assert_eq!(ctx.zoom_factor(), 1.0);
    }
}
