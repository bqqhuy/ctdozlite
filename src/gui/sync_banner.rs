//! Sync Banner V2 with typed state machine, parity with Zalo PC native `pA`,
//! strict popup suppression, no UI stubs, and fencing protection against stale clicks.

use std::time::Instant;

use eframe::egui::{self, Color32, Margin, RichText, Rounding, Stroke, Vec2};

use crate::gui::theme::ThemePalette;
use crate::gui::theme::{draw_lucide_icon, lucide};
use crate::sync_coordinator::{SyncState, SyncTransferStage};

// ----------------------------------------------------------------------------
// Authoritative Zalo PC native string keys and Vietnamese display labels
// (Parity with compact-app-pc.js:13200000-13205000 and lang-vi.js)
// ----------------------------------------------------------------------------

pub const STR_SYNC_BANNER_COMPACT_SUGGEST_TITLE: &str = "Đồng bộ tin nhắn gần đây?";
pub const STR_SYNC_BANNER_COMPACT_SUGGEST_CTA: &str = "Đồng bộ ngay";
pub const STR_SYNC_IN_PROGRESS_BANNER_LABEL: &str = "Đang đồng bộ tin nhắn...";
pub const STR_SYNC_BANNER_INTERRUPT_TITLE: &str = "Đồng bộ bị gián đoạn";
pub const STR_SYNC_BANNER_COMPACT_INTERRUPT_DESCRIPTION: &str =
    "Mở Zalo trên điện thoại để tiếp tục";
pub const STR_SYNC_BANNER_COMPACT_ERROR_TITLE: &str = "Đồng bộ không thành công";
pub const STR_SYNC_BANNER_COMPACT_RETRY_CTA: &str = "Thử lại";
pub const STR_SYNC_BANNER_COMPACT_SUCCESS_TITLE: &str = "Đã đồng bộ tin nhắn";

/// Fencing token capturing operation identity and state revision to reject stale UI clicks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncBannerFencing {
    pub owner: String,
    pub runtime_id: u64,
    pub operation_id: u64,
    pub revision: u64,
}

impl SyncBannerFencing {
    pub fn new(
        owner: impl Into<String>,
        runtime_id: u64,
        operation_id: u64,
        revision: u64,
    ) -> Self {
        Self {
            owner: owner.into(),
            runtime_id,
            operation_id,
            revision,
        }
    }

    /// Verifies if a user action matches the current active app state and revision.
    pub fn is_valid(
        &self,
        current_owner: &str,
        current_runtime_id: u64,
        current_revision: u64,
    ) -> bool {
        !self.owner.is_empty()
            && self.owner == current_owner
            && self.runtime_id == current_runtime_id
            && self.revision == current_revision
    }
}

/// Suggestion triggers recognized by Zalo PC native (`canShowSyncSuggestion`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SyncSuggestionTrigger {
    NewLogIn,
    ChangeDevice,
    QueueEvict,
    Persisted,
}

/// Detects errors that must not offer retry (e.g. database migration, permission denied, fatal, revoked).
pub fn is_non_retryable_error(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("dbmigrat")
        || lower.contains("permissiondenied")
        || lower.contains("quyền bị từ chối")
        || lower.contains("sessionexpired")
        || lower.contains("hết hạn")
        || lower.contains("revoked")
        || lower.contains("thu hồi")
        || lower.contains("unsupported_version")
        || lower.contains("không tương thích")
        || lower.contains("device_rejected")
        || lower.contains("fatal")
        || lower.contains("corrupt")
}

/// Typed representation of the eight distinct banner states in Zalo PC V2 (`pA`).
#[derive(Clone, Debug, PartialEq)]
pub enum SyncBannerKind {
    /// Suggesting synchronization to the user.
    Suggest {
        title: Option<String>,
        cta: Option<String>,
        description: Option<String>,
    },
    /// Waiting for user confirmation on mobile device with timeout deadline.
    WaitConfirmation { timeout_at: Instant },
    /// Preparing synchronization metadata and connection.
    Preparing,
    /// Syncing conversation list and recent preview messages.
    SyncingConvListRecent { detail: Option<String> },
    /// Syncing remaining historical messages with progress percentage (only when known).
    SyncingOtherMessages {
        percentage: Option<f32>,
        detail: Option<String>,
    },
    /// Synchronization was interrupted or paused.
    Interrupted {
        description: String,
        retryable: bool,
    },
    /// Synchronization error requiring retry or dismissal.
    Error { message: String, can_retry: bool },
    /// Successful synchronization with auto-dismiss deadline.
    Success { deadline: Instant, message: String },
}

impl SyncBannerKind {
    pub fn is_running(&self) -> bool {
        matches!(
            self,
            Self::WaitConfirmation { .. }
                | Self::Preparing
                | Self::SyncingConvListRecent { .. }
                | Self::SyncingOtherMessages { .. }
        )
    }

    pub fn title(&self) -> &str {
        match self {
            Self::Suggest { title: Some(t), .. } => t.as_str(),
            Self::Suggest { title: None, .. } => STR_SYNC_BANNER_COMPACT_SUGGEST_TITLE,
            Self::WaitConfirmation { .. } => "Chờ xác nhận từ điện thoại…",
            Self::Preparing => "Đang chuẩn bị đồng bộ…",
            Self::SyncingConvListRecent { .. } => STR_SYNC_IN_PROGRESS_BANNER_LABEL,
            Self::SyncingOtherMessages { .. } => STR_SYNC_IN_PROGRESS_BANNER_LABEL,
            Self::Interrupted { .. } => STR_SYNC_BANNER_INTERRUPT_TITLE,
            Self::Error { .. } => STR_SYNC_BANNER_COMPACT_ERROR_TITLE,
            Self::Success { .. } => STR_SYNC_BANNER_COMPACT_SUCCESS_TITLE,
        }
    }
}

/// Action produced by interactive banner controls to be dispatched to Worker and App State.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncBannerAction {
    /// "Đồng bộ ngay" clicked on Suggest banner.
    StartSuggestSync,
    /// "Thử lại" clicked on Error banner.
    RetryFailedSync,
    /// "Dừng" clicked on Running / Interrupted / WaitConfirmation banner.
    CancelSync,
    /// Close [x] button clicked: classified by active banner kind.
    DismissOrClose,
}

/// Inputs controlling sync banner resolution.
#[derive(Clone, Copy, Debug)]
pub struct SyncBannerInput<'a> {
    pub is_logged_in: bool,
    pub popup_visible: bool,
    pub suggest_dismissed: bool,
    pub error_dismissed: bool,
    pub success_until: Option<Instant>,
    pub legacy_running: bool,
    pub legacy_detail: Option<&'a str>,
    pub suggestion_trigger: Option<SyncSuggestionTrigger>,
    pub sync_config: Option<&'a crate::server_config::Sync2PcConfig>,
}

/// Pure state resolver that determines the active banner kind according to Zalo PC native rules.
///
/// Invariant:
/// - If `!is_logged_in` or `popup_visible`, returns `None` (Popup Suppression Invariant).
/// - Error takes priority over idle/interrupted. Non-retryable errors suppress retry CTA.
/// - Success is shown until `success_until` deadline.
/// - Idle only suggests when a valid trigger is active and allowed by server config.
/// - Percentages are never fabricated when the denominator is unknown.
pub(super) fn resolve_banner_state(
    input: &SyncBannerInput<'_>,
    sync_state: &SyncState,
) -> Option<SyncBannerKind> {
    // Popup Suppression Invariant (parity with native `pA: if (n.popupVisible) return null`)
    if !input.is_logged_in || input.popup_visible {
        return None;
    }

    // New banner v2 gate: when new_banner_sync_v2 is explicitly disabled, do not render banner v2
    if let Some(cfg) = input.sync_config {
        if !cfg.new_banner_sync_v2 {
            return None;
        }
    }

    // 1. Error state (sA in native)
    if let SyncState::Failed { error, fatal } = sync_state {
        if !input.error_dismissed {
            let can_retry = if *fatal {
                false
            } else {
                !is_non_retryable_error(error)
            };
            return Some(SyncBannerKind::Error {
                message: error.clone(),
                can_retry,
            });
        }
    }

    // 2. Interrupted state (hA in native)
    if let SyncState::Interrupted { reason, retryable } = sync_state {
        return Some(SyncBannerKind::Interrupted {
            description: if reason.is_empty() {
                STR_SYNC_BANNER_COMPACT_INTERRUPT_DESCRIPTION.to_string()
            } else {
                reason.clone()
            },
            retryable: *retryable,
        });
    }

    // 3. Success state (oA in native)
    if let Some(deadline) = input.success_until {
        if Instant::now() < deadline {
            let msg = if let SyncState::Completed {
                inserted_messages,
                duration,
            } = sync_state
            {
                format!(
                    "{inserted_messages} tin mới ({:.1}s)",
                    duration.as_secs_f32()
                )
            } else {
                "Đã hoàn tất đồng bộ tin nhắn".to_string()
            };
            return Some(SyncBannerKind::Success {
                deadline,
                message: msg,
            });
        }
    }

    // 4. Running states
    match sync_state {
        SyncState::AwaitingPhoneConfirmation { timeout_at } => {
            return Some(SyncBannerKind::WaitConfirmation {
                timeout_at: *timeout_at,
            });
        }
        SyncState::Preparing { .. } => {
            return Some(SyncBannerKind::Preparing);
        }
        SyncState::Transferring { stage, .. } => {
            if *stage == SyncTransferStage::DescriptorReceived {
                return Some(SyncBannerKind::SyncingConvListRecent {
                    detail: Some(sync_state.summary_vi()),
                });
            } else {
                return Some(SyncBannerKind::SyncingOtherMessages {
                    percentage: sync_state.known_percentage(),
                    detail: Some(sync_state.summary_vi()),
                });
            }
        }
        _ => {}
    }

    // Legacy running fallback (e.g. background history sync)
    if input.legacy_running {
        return Some(SyncBannerKind::SyncingConvListRecent {
            detail: input.legacy_detail.map(str::to_string),
        });
    }

    // 5. Suggest state (AA in native: canShowSyncSuggestion)
    if let SyncState::Idle { availability } = sync_state {
        if availability.is_ready() && !input.suggest_dismissed {
            if let Some(trigger) = input.suggestion_trigger {
                let enabled = match input.sync_config {
                    Some(cfg) => {
                        cfg.enable
                            && match trigger {
                                SyncSuggestionTrigger::NewLogIn => {
                                    cfg.sync_suggestion_zalopc.new_log_in
                                }
                                SyncSuggestionTrigger::ChangeDevice => {
                                    cfg.sync_suggestion_zalopc.change_device
                                }
                                SyncSuggestionTrigger::QueueEvict => {
                                    cfg.sync_suggestion_zalopc.queue_evict
                                }
                                SyncSuggestionTrigger::Persisted => true,
                            }
                    }
                    None => true,
                };
                if enabled {
                    let (title, cta, description) = match input.sync_config {
                        Some(cfg) => {
                            let text = &cfg.sync_suggestion_zalopc.banner_text;
                            let title = if !text.title.vi.is_empty() {
                                Some(text.title.vi.clone())
                            } else {
                                None
                            };
                            let cta = if !text.cta.vi.is_empty() {
                                Some(text.cta.vi.clone())
                            } else {
                                None
                            };
                            let description = (!text.description.vi.is_empty())
                                .then(|| text.description.vi.clone());
                            (title, cta, description)
                        }
                        None => (None, None, None),
                    };
                    return Some(SyncBannerKind::Suggest {
                        title,
                        cta,
                        description,
                    });
                }
            }
        }
    }

    None
}

/// Renders the sync banner into the provided Ui and returns an action if a button was clicked.
/// Guarantees strict layout containment so sidebar width is never widened.
pub(super) fn render_sync_banner(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    pal: &ThemePalette,
    kind: &SyncBannerKind,
    _fencing: &SyncBannerFencing,
    cancel_requested: bool,
) -> Option<SyncBannerAction> {
    let mut triggered_action = None;

    // Palette-informed styling for dark/light themes
    let (bg_color, border_color, icon_str) = match kind {
        SyncBannerKind::Error { .. } => {
            if pal.is_dark {
                (
                    Color32::from_rgb(50, 24, 24),
                    Color32::from_rgb(130, 45, 45),
                    "⚠",
                )
            } else {
                (
                    Color32::from_rgb(254, 242, 242),
                    Color32::from_rgb(254, 202, 202),
                    "⚠",
                )
            }
        }
        SyncBannerKind::Success { .. } => {
            // Keep repainting until the success deadline expires so it auto-hides smoothly
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
            if pal.is_dark {
                (
                    Color32::from_rgb(20, 40, 28),
                    Color32::from_rgb(34, 100, 50),
                    "✓",
                )
            } else {
                (
                    Color32::from_rgb(236, 253, 245),
                    Color32::from_rgb(167, 243, 208),
                    "✓",
                )
            }
        }
        SyncBannerKind::Interrupted { .. } => {
            if pal.is_dark {
                (
                    Color32::from_rgb(45, 36, 20),
                    Color32::from_rgb(120, 90, 30),
                    "⏸",
                )
            } else {
                (
                    Color32::from_rgb(254, 243, 199),
                    Color32::from_rgb(253, 230, 138),
                    "⏸",
                )
            }
        }
        SyncBannerKind::WaitConfirmation { timeout_at } => {
            let now = Instant::now();
            if now < *timeout_at {
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
            if pal.is_dark {
                (
                    Color32::from_rgb(22, 36, 56),
                    Color32::from_rgb(37, 85, 135),
                    "",
                )
            } else {
                (
                    Color32::from_rgb(239, 246, 255),
                    Color32::from_rgb(191, 219, 254),
                    "",
                )
            }
        }
        SyncBannerKind::Preparing
        | SyncBannerKind::SyncingConvListRecent { .. }
        | SyncBannerKind::SyncingOtherMessages { .. } => {
            if pal.is_dark {
                (
                    Color32::from_rgb(22, 36, 56),
                    Color32::from_rgb(37, 85, 135),
                    "",
                )
            } else {
                (
                    Color32::from_rgb(239, 246, 255),
                    Color32::from_rgb(191, 219, 254),
                    "",
                )
            }
        }
        SyncBannerKind::Suggest { .. } => (pal.bg_elevated, pal.border, lucide::REFRESH),
    };

    let title_text = match kind {
        SyncBannerKind::SyncingOtherMessages {
            percentage: Some(pct),
            ..
        } => format!("{} ({:.0}%)", STR_SYNC_IN_PROGRESS_BANNER_LABEL, pct),
        SyncBannerKind::WaitConfirmation { timeout_at } => {
            let remaining = timeout_at
                .saturating_duration_since(Instant::now())
                .as_secs();
            format!("Chờ xác nhận điện thoại ({remaining}s)…")
        }
        _ => kind.title().to_string(),
    };
    let tooltip = match kind {
        SyncBannerKind::Error { message, .. } | SyncBannerKind::Success { message, .. } => {
            message.clone()
        }
        SyncBannerKind::Interrupted { description, .. } => description.clone(),
        SyncBannerKind::SyncingConvListRecent {
            detail: Some(detail),
        }
        | SyncBannerKind::SyncingOtherMessages {
            detail: Some(detail),
            ..
        } => detail.clone(),
        _ => title_text.clone(),
    };
    let action = match kind {
        SyncBannerKind::Suggest { cta, .. } => Some((
            cta.as_deref()
                .unwrap_or(STR_SYNC_BANNER_COMPACT_SUGGEST_CTA),
            SyncBannerAction::StartSuggestSync,
            true,
        )),
        SyncBannerKind::Error {
            can_retry: true, ..
        } => Some((
            STR_SYNC_BANNER_COMPACT_RETRY_CTA,
            SyncBannerAction::RetryFailedSync,
            true,
        )),
        SyncBannerKind::Interrupted {
            retryable: true, ..
        } => Some((
            STR_SYNC_BANNER_COMPACT_RETRY_CTA,
            SyncBannerAction::RetryFailedSync,
            true,
        )),
        _ => None,
    };
    let compact = ui.available_width() < 140.0;
    egui::Frame::none()
        .fill(bg_color)
        .stroke(Stroke::new(1.0_f32, border_color))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(if compact { 4.0 } else { 10.0 }, 8.0))
        .show(ui, |ui| {
            if compact {
                // The collapsed sidebar has no text rows; retain the same actions as icon targets.
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.spacing_mut().button_padding = Vec2::splat(2.0);
                    if let Some((label, intent, enabled)) = action {
                        let icon = if intent == SyncBannerAction::CancelSync {
                            lucide::BAN
                        } else {
                            lucide::REFRESH
                        };
                        if ui
                            .add_enabled(
                                enabled,
                                egui::Button::new(RichText::new(icon).size(14.0))
                                    .min_size(Vec2::splat(24.0))
                                    .frame(false),
                            )
                            .on_hover_text(format!("{title_text}\n{label}\n{tooltip}"))
                            .clicked()
                        {
                            triggered_action = Some(intent);
                        }
                    } else if kind.is_running() {
                        ui.add_sized(Vec2::splat(24.0), egui::Spinner::new().size(14.0))
                            .on_hover_text(&tooltip);
                    } else {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(24.0), egui::Sense::hover());
                        draw_lucide_icon(
                            ui.painter(),
                            rect.center(),
                            if matches!(kind, SyncBannerKind::Error { .. }) {
                                lucide::BAN
                            } else {
                                lucide::CHECK
                            },
                            14.0,
                            pal.text_secondary,
                        );
                        response.on_hover_text(&tooltip);
                    }
                    if ui
                        .add_sized(
                            Vec2::splat(24.0),
                            egui::Button::new(RichText::new(lucide::X).size(14.0)).frame(false),
                        )
                        .on_hover_text(if kind.is_running() {
                            if cancel_requested {
                                "Đang dừng đồng bộ…"
                            } else {
                                "Dừng đồng bộ"
                            }
                        } else {
                            "Đóng thông báo đồng bộ"
                        })
                        .clicked()
                    {
                        triggered_action = Some(SyncBannerAction::DismissOrClose);
                    }
                });
                return;
            }

            // Expanded sidebar view
            ui.vertical(|ui| {
                // Row 1: Status / Title row with leading icon and right-aligned close button
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if kind.is_running() {
                        ui.spinner();
                    } else if !icon_str.is_empty() {
                        let color = match kind {
                            SyncBannerKind::Error { .. } => pal.danger,
                            SyncBannerKind::Success { .. } => pal.accent_online,
                            SyncBannerKind::Interrupted { .. } => pal.warning,
                            _ => pal.primary,
                        };
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                        if matches!(kind, SyncBannerKind::Suggest { .. }) {
                            draw_lucide_icon(ui.painter(), rect.center(), icon_str, 14.0, color);
                        } else {
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                icon_str,
                                egui::FontId::proportional(14.0),
                                color,
                            );
                        }
                    }

                    let right_btn_w = 28.0;
                    let title_width = (ui.available_width() - right_btn_w - 4.0).max(0.0);
                    let (title_rect, _) =
                        ui.allocate_exact_size(Vec2::new(title_width, 24.0), egui::Sense::hover());
                    let mut title_ui = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(title_rect)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    title_ui
                        .add(
                            egui::Label::new(
                                RichText::new(&title_text)
                                    .size(12.0)
                                    .strong()
                                    .color(pal.text_primary),
                            )
                            .truncate(),
                        )
                        .on_hover_text(&tooltip);

                    let close_tooltip = match kind {
                        SyncBannerKind::Error { .. } => "Đóng và xóa thông báo lỗi",
                        SyncBannerKind::Suggest { .. } => "Ẩn gợi ý đồng bộ",
                        SyncBannerKind::Success { .. } => "Đóng thông báo",
                        _ if cancel_requested => "Đang dừng đồng bộ…",
                        _ => "Dừng đồng bộ",
                    };
                    if ui
                        .add_sized(
                            Vec2::splat(24.0),
                            egui::Button::new(RichText::new(lucide::X).size(13.0)).frame(false),
                        )
                        .on_hover_text(close_tooltip)
                        .clicked()
                    {
                        triggered_action = Some(SyncBannerAction::DismissOrClose);
                    }
                });

                if let SyncBannerKind::Suggest {
                    description: Some(description),
                    ..
                } = kind
                {
                    ui.add_space(3.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new(description)
                                .size(11.0)
                                .color(pal.text_secondary),
                        )
                        .wrap(),
                    );
                }
                if let Some((label, intent, enabled)) = action {
                    ui.add_space(6.0);
                    let suggestion = matches!(kind, SyncBannerKind::Suggest { .. });
                    let text = RichText::new(label).size(12.0).color(if suggestion {
                        pal.primary
                    } else {
                        Color32::WHITE
                    });
                    let mut button = egui::Button::new(if suggestion {
                        text.underline()
                    } else {
                        text.strong()
                    })
                    .wrap()
                    .frame(!suggestion)
                    .rounding(Rounding::same(6.0));
                    if !suggestion {
                        button = button.fill(pal.primary);
                    }
                    ui.add_enabled_ui(enabled, |ui| {
                        let response = ui
                            .add_sized(Vec2::new(ui.available_width(), 32.0), button)
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        #[cfg(test)]
                        ctx.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::new("sync-suggest-action-bounds"),
                                response.rect,
                            )
                        });
                        if response.clicked() {
                            triggered_action = Some(intent);
                        }
                    });
                }
            });
        });

    triggered_action
}

// ----------------------------------------------------------------------------
// Unit Tests: Complete state transitions, stale action rejection, popup suppression,
// and close button classification
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_coordinator::SyncAvailability;
    use std::time::Duration;

    #[test]
    fn suggestion_action_has_readable_height_and_title_is_left_aligned() {
        for dark in [false, true] {
            for width in [160.0, 240.0, 340.0] {
                let ctx = egui::Context::default();
                let pal = if dark {
                    ThemePalette::dark()
                } else {
                    ThemePalette::light()
                };
                pal.apply_to_ctx(&ctx);
                let title = "Đồng bộ tin nhắn gần đây?";
                let cta = "Nhấn để đồng bộ ngay";
                let output = ctx.run(Default::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let bounds =
                            egui::Rect::from_min_size(ui.cursor().min, Vec2::new(width, 500.0));
                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                        render_sync_banner(
                            &mut child,
                            ctx,
                            &pal,
                            &SyncBannerKind::Suggest {
                                title: Some(title.into()),
                                cta: Some(cta.into()),
                                description: None,
                            },
                            &SyncBannerFencing::new("fixture", 1, 1, 1),
                            false,
                        );
                    });
                });
                let action_rect = ctx.data(|data| {
                    data.get_temp::<egui::Rect>(egui::Id::new("sync-suggest-action-bounds"))
                });
                let mut title_rect = None;
                let mut action_text_rect = None;
                for shape in &output.shapes {
                    match &shape.shape {
                        egui::Shape::Text(text) if text.galley.job.text == title => {
                            title_rect =
                                Some(egui::Rect::from_min_size(text.pos, text.galley.size()));
                        }
                        egui::Shape::Text(text) if text.galley.job.text == cta => {
                            action_text_rect =
                                Some(egui::Rect::from_min_size(text.pos, text.galley.size()));
                        }
                        _ => {}
                    }
                }
                let action = action_rect.expect("suggestion action must have a hit target");
                assert!(action.height() >= 32.0, "{width}: {action:?}");
                assert!(action.contains_rect(action_text_rect.expect("CTA must be painted")));
                let title_rect = title_rect.expect("title must be painted");
                assert!(
                    title_rect.left() <= action.left() + 24.0,
                    "title drifted to the center: {title_rect:?}"
                );
            }
        }
    }

    #[test]
    fn configured_cta_and_all_banner_states_stay_inside_parent_width() {
        for scale in [1.0, 1.5, 2.0] {
            for width in [80.0, 160.0, 260.0, 340.0] {
                for kind in [
                    SyncBannerKind::Suggest {
                        title: None,
                        cta: Some("Nhấn để đồng bộ ngay với điện thoại của bạn".into()),
                        description: None,
                    },
                    SyncBannerKind::Preparing,
                    SyncBannerKind::WaitConfirmation {
                        timeout_at: Instant::now() + Duration::from_secs(20),
                    },
                    SyncBannerKind::SyncingConvListRecent { detail: None },
                    SyncBannerKind::SyncingOtherMessages {
                        percentage: Some(50.0),
                        detail: None,
                    },
                    SyncBannerKind::Error {
                        message: "Lỗi".into(),
                        can_retry: true,
                    },
                    SyncBannerKind::Interrupted {
                        description: "Gián đoạn".into(),
                        retryable: true,
                    },
                    SyncBannerKind::Success {
                        deadline: Instant::now(),
                        message: "Xong".into(),
                    },
                ] {
                    let ctx = egui::Context::default();
                    ThemePalette::light().apply_to_ctx(&ctx);
                    ctx.set_pixels_per_point(scale);
                    for _ in 0..3 {
                        let _ = ctx.run(Default::default(), |ctx| {
                            egui::CentralPanel::default().show(ctx, |ui| {
                                let bounds = egui::Rect::from_min_size(
                                    ui.cursor().min,
                                    Vec2::new(width, 500.0),
                                );
                                let mut child =
                                    ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                                render_sync_banner(
                                    &mut child,
                                    ctx,
                                    &ThemePalette::light(),
                                    &kind,
                                    &SyncBannerFencing::new("owner", 1, 1, 1),
                                    false,
                                );
                                assert!(
                                    child.min_rect().right() <= bounds.right() + 1.0,
                                    "{width}@{scale}: {kind:?}: {:?}",
                                    child.min_rect()
                                );
                                if width < 140.0 {
                                    assert!(child.min_rect().height() < 60.0,
                                        "collapsed sidebar must not stack CTA words: {kind:?}: {:?}", child.min_rect());
                                }
                            });
                        });
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn test_resolve(
        is_logged_in: bool,
        popup_visible: bool,
        suggest_dismissed: bool,
        error_dismissed: bool,
        success_until: Option<Instant>,
        sync_state: &SyncState,
        legacy_running: bool,
        legacy_detail: Option<&str>,
    ) -> Option<SyncBannerKind> {
        let input = SyncBannerInput {
            is_logged_in,
            popup_visible,
            suggest_dismissed,
            error_dismissed,
            success_until,
            legacy_running,
            legacy_detail,
            suggestion_trigger: Some(SyncSuggestionTrigger::NewLogIn),
            sync_config: None,
        };
        resolve_banner_state(&input, sync_state)
    }

    #[test]
    fn resolve_banner_state_popup_suppression_invariant() {
        let state = SyncState::Preparing {
            source: crate::sync_coordinator::SyncSource::Sync2Full,
        };
        // When modal/popup is visible, banner MUST BE HIDDEN (None)
        assert_eq!(
            test_resolve(true, true, false, false, None, &state, false, None),
            None
        );
        // When popup is closed, banner appears
        assert_eq!(
            test_resolve(true, false, false, false, None, &state, false, None),
            Some(SyncBannerKind::Preparing)
        );
        // When not logged in, banner is hidden
        assert_eq!(
            test_resolve(false, false, false, false, None, &state, false, None),
            None
        );
    }

    #[test]
    fn resolve_banner_state_transitions_through_full_native_lifecycle() {
        // 1. Suggest when Idle and not dismissed
        let idle_state = SyncState::Idle {
            availability: SyncAvailability::Ready,
        };
        assert_eq!(
            test_resolve(true, false, false, false, None, &idle_state, false, None),
            Some(SyncBannerKind::Suggest {
                title: None,
                cta: None,
                description: None,
            })
        );
        // Idle when Unavailable MUST NEVER show Suggest banner
        let unavail_state = SyncState::Idle {
            availability: SyncAvailability::Unavailable(
                crate::sync_coordinator::SyncUnavailableReason::WorkerNotReady,
            ),
        };
        assert_eq!(
            test_resolve(true, false, false, false, None, &unavail_state, false, None),
            None
        );
        // Idle when trigger is None MUST NEVER show Suggest banner
        let no_trigger_input = SyncBannerInput {
            is_logged_in: true,
            popup_visible: false,
            suggest_dismissed: false,
            error_dismissed: false,
            success_until: None,
            legacy_running: false,
            legacy_detail: None,
            suggestion_trigger: None,
            sync_config: None,
        };
        assert_eq!(resolve_banner_state(&no_trigger_input, &idle_state), None);

        // Dismissing suggest hides it
        assert_eq!(
            test_resolve(true, false, true, false, None, &idle_state, false, None),
            None
        );

        // 2. WaitConfirmation
        let deadline = Instant::now() + Duration::from_secs(60);
        let wait_state = SyncState::AwaitingPhoneConfirmation {
            timeout_at: deadline,
        };
        assert_eq!(
            test_resolve(true, false, false, false, None, &wait_state, false, None),
            Some(SyncBannerKind::WaitConfirmation {
                timeout_at: deadline
            })
        );

        // 3. Preparing
        let prep_state = SyncState::Preparing {
            source: crate::sync_coordinator::SyncSource::Sync2Recent,
        };
        assert_eq!(
            test_resolve(true, false, false, false, None, &prep_state, false, None),
            Some(SyncBannerKind::Preparing)
        );

        // 4. SyncingConvListRecent
        let conv_state = SyncState::Transferring {
            stage: SyncTransferStage::DescriptorReceived,
            downloaded_bytes: 0,
            total_bytes: None,
            inserted_messages: 0,
            total_messages: None,
        };
        let res_conv = test_resolve(true, false, false, false, None, &conv_state, false, None);
        assert!(matches!(
            res_conv,
            Some(SyncBannerKind::SyncingConvListRecent { .. })
        ));

        // 5. SyncingOtherMessages with known percentage
        let other_state = SyncState::Transferring {
            stage: SyncTransferStage::Downloaded,
            downloaded_bytes: 50,
            total_bytes: Some(100),
            inserted_messages: 0,
            total_messages: None,
        };
        let res_other = test_resolve(true, false, false, false, None, &other_state, false, None);
        assert_eq!(
            res_other,
            Some(SyncBannerKind::SyncingOtherMessages {
                percentage: Some(50.0),
                detail: Some(other_state.summary_vi()),
            })
        );

        // 5b. SyncingOtherMessages without known percentage (NEVER fabricates fake %)
        let other_unknown = SyncState::Transferring {
            stage: SyncTransferStage::DbCommitted,
            downloaded_bytes: 1024,
            total_bytes: None,
            inserted_messages: 10,
            total_messages: None,
        };
        let res_unk = test_resolve(true, false, false, false, None, &other_unknown, false, None);
        assert_eq!(
            res_unk,
            Some(SyncBannerKind::SyncingOtherMessages {
                percentage: None,
                detail: Some(other_unknown.summary_vi()),
            })
        );

        // 6. Interrupted
        let inter_state = SyncState::Interrupted {
            reason: "Mất kết nối".to_string(),
            retryable: true,
        };
        assert_eq!(
            test_resolve(true, false, false, false, None, &inter_state, false, None),
            Some(SyncBannerKind::Interrupted {
                description: "Mất kết nối".to_string(),
                retryable: true,
            })
        );

        // 7. Error
        let err_state = SyncState::Failed {
            error: "Hỏng CSDL".to_string(),
            fatal: false,
        };
        assert_eq!(
            test_resolve(true, false, false, false, None, &err_state, false, None),
            Some(SyncBannerKind::Error {
                message: "Hỏng CSDL".to_string(),
                can_retry: true,
            })
        );
        // Dismissing error hides it
        assert_eq!(
            test_resolve(true, false, false, true, None, &err_state, false, None),
            None
        );

        // 8. Success
        let success_deadline = Instant::now() + Duration::from_secs(3);
        let comp_state = SyncState::Completed {
            inserted_messages: 42,
            duration: Duration::from_secs(2),
        };
        assert_eq!(
            test_resolve(
                true,
                false,
                false,
                false,
                Some(success_deadline),
                &comp_state,
                false,
                None
            ),
            Some(SyncBannerKind::Success {
                deadline: success_deadline,
                message: "42 tin mới (2.0s)".to_string(),
            })
        );
        // Expired success deadline auto-hides completed state
        let past_deadline = Instant::now() - Duration::from_secs(1);
        assert_eq!(
            test_resolve(
                true,
                false,
                false,
                false,
                Some(past_deadline),
                &comp_state,
                false,
                None
            ),
            None
        );
        // Once state is reset to Idle and not dismissed, suggest banner appears
        assert_eq!(
            test_resolve(
                true,
                false,
                false,
                false,
                Some(past_deadline),
                &idle_state,
                false,
                None
            ),
            Some(SyncBannerKind::Suggest {
                title: None,
                cta: None,
                description: None,
            })
        );
    }

    #[test]
    fn non_retryable_errors_suppress_retry_button() {
        let fatal_state = SyncState::Failed {
            error: "SQLite migration failed (DbMigrating)".to_string(),
            fatal: true,
        };
        let res = test_resolve(true, false, false, false, None, &fatal_state, false, None);
        assert_eq!(
            res,
            Some(SyncBannerKind::Error {
                message: "SQLite migration failed (DbMigrating)".to_string(),
                can_retry: false,
            })
        );

        // Even with a generic error message, if fatal: true, can_retry MUST BE false!
        let generic_fatal = SyncState::Failed {
            error: "something went wrong".to_string(),
            fatal: true,
        };
        let res2 = test_resolve(true, false, false, false, None, &generic_fatal, false, None);
        assert_eq!(
            res2,
            Some(SyncBannerKind::Error {
                message: "something went wrong".to_string(),
                can_retry: false,
            })
        );
    }

    #[test]
    fn new_banner_sync_v2_flag_gates_banner_rendering() {
        let cfg = crate::server_config::Sync2PcConfig {
            new_banner_sync_v2: false,
            ..Default::default()
        };
        let idle_state = SyncState::Idle {
            availability: SyncAvailability::Ready,
        };
        let input = SyncBannerInput {
            is_logged_in: true,
            popup_visible: false,
            suggest_dismissed: false,
            error_dismissed: false,
            success_until: None,
            legacy_running: false,
            legacy_detail: None,
            suggestion_trigger: Some(SyncSuggestionTrigger::NewLogIn),
            sync_config: Some(&cfg),
        };
        assert_eq!(
            resolve_banner_state(&input, &idle_state),
            None,
            "new_banner_sync_v2 = false must suppress banner v2"
        );
    }

    #[test]
    fn custom_banner_text_and_triggers_respected() {
        let mut cfg = crate::server_config::Sync2PcConfig::default();
        cfg.sync_suggestion_zalopc.banner_text.title.vi = "Đồng bộ từ điện thoại của bạn".into();
        cfg.sync_suggestion_zalopc.banner_text.cta.vi = "Bắt đầu".into();
        cfg.sync_suggestion_zalopc.banner_text.description.vi = "Mô tả thử nghiệm".into();
        cfg.sync_suggestion_zalopc.change_device = false;

        let idle_state = SyncState::Idle {
            availability: SyncAvailability::Ready,
        };
        let input_new = SyncBannerInput {
            is_logged_in: true,
            popup_visible: false,
            suggest_dismissed: false,
            error_dismissed: false,
            success_until: None,
            legacy_running: false,
            legacy_detail: None,
            suggestion_trigger: Some(SyncSuggestionTrigger::NewLogIn),
            sync_config: Some(&cfg),
        };
        assert_eq!(
            resolve_banner_state(&input_new, &idle_state),
            Some(SyncBannerKind::Suggest {
                title: Some("Đồng bộ từ điện thoại của bạn".into()),
                cta: Some("Bắt đầu".into()),
                description: Some("Mô tả thử nghiệm".into()),
            })
        );

        let input_change = SyncBannerInput {
            suggestion_trigger: Some(SyncSuggestionTrigger::ChangeDevice),
            ..input_new
        };
        assert_eq!(resolve_banner_state(&input_change, &idle_state), None);
    }

    #[test]
    fn fencing_token_rejects_stale_actions() {
        let fencing = SyncBannerFencing::new("user_123", 1, 10, 5);

        // Valid matching context
        assert!(fencing.is_valid("user_123", 1, 5));

        // Mismatched owner (account switched)
        assert!(!fencing.is_valid("user_456", 1, 5));

        // Mismatched runtime_id (session restarted/switched)
        assert!(!fencing.is_valid("user_123", 2, 5));

        // Mismatched revision (state transitioned since banner was painted)
        assert!(!fencing.is_valid("user_123", 1, 6));
    }
}
