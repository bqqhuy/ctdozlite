//! Sync Banner V2 with typed state machine, parity with Zalo PC native `pA`,
//! strict popup suppression, no UI stubs, and fencing protection against stale clicks.

use std::time::Instant;

use eframe::egui::{self, Color32, Margin, RichText, Rounding, Stroke, Vec2};

use crate::gui::theme::ThemePalette;
use crate::sync_coordinator::{SyncAvailability, SyncState, SyncTransferStage};

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

/// Typed representation of the eight distinct banner states in Zalo PC V2 (`pA`).
#[derive(Clone, Debug, PartialEq)]
pub enum SyncBannerKind {
    /// Suggesting synchronization to the user.
    Suggest,
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

    pub fn title(&self) -> &'static str {
        match self {
            Self::Suggest => STR_SYNC_BANNER_COMPACT_SUGGEST_TITLE,
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
}

/// Pure state resolver that determines the active banner kind according to Zalo PC native rules.
///
/// Invariant:
/// - If `!is_logged_in` or `popup_visible`, returns `None` (Popup Suppression Invariant).
/// - Error takes priority over idle/interrupted.
/// - Success is shown until `success_until` deadline.
/// - Percentages are never fabricated when the denominator is unknown.
pub(super) fn resolve_banner_state(
    input: &SyncBannerInput<'_>,
    sync_state: &SyncState,
) -> Option<SyncBannerKind> {
    // Popup Suppression Invariant (parity with native `pA: if (n.popupVisible) return null`)
    if !input.is_logged_in || input.popup_visible {
        return None;
    }

    // 1. Error state (sA in native)
    if let SyncState::Failed { error, .. } = sync_state {
        if !input.error_dismissed {
            return Some(SyncBannerKind::Error {
                message: error.clone(),
                can_retry: true,
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

    // 5. Suggest state (AA in native)
    if (matches!(
        sync_state,
        SyncState::Idle {
            availability: SyncAvailability::Ready
        }
    ) || matches!(sync_state, SyncState::Idle { .. }))
        && !input.suggest_dismissed
    {
        return Some(SyncBannerKind::Suggest);
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
        SyncBannerKind::Suggest => (pal.bg_elevated, pal.border, "↺"),
    };

    let avail_width = ui.available_width().max(1.0);

    egui::Frame::none()
        .fill(bg_color)
        .stroke(Stroke::new(1.0_f32, border_color))
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(8.0, 6.0))
        .show(ui, |ui| {
            // Constrain layout so label text cannot expand the container width
            ui.set_max_width(avail_width);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;

                // 1. Icon / Spinner
                if kind.is_running() {
                    ui.spinner();
                } else if !icon_str.is_empty() {
                    let icon_color = match kind {
                        SyncBannerKind::Error { .. } => pal.danger,
                        SyncBannerKind::Success { .. } => pal.accent_online,
                        SyncBannerKind::Interrupted { .. } => pal.warning,
                        _ => pal.primary,
                    };
                    ui.colored_label(icon_color, icon_str);
                }

                // 2. Action button width reservation
                let has_cta =
                    matches!(kind, SyncBannerKind::Suggest | SyncBannerKind::Error { .. });
                let has_stop =
                    kind.is_running() || matches!(kind, SyncBannerKind::Interrupted { .. });
                let action_width = if has_cta {
                    76.0
                } else if has_stop {
                    54.0
                } else {
                    0.0
                };
                let close_btn_width = 20.0;
                let spacing_total = 20.0;
                let label_width =
                    (ui.available_width() - action_width - close_btn_width - spacing_total)
                        .max(1.0);

                // 3. Text block
                ui.allocate_ui_with_layout(
                    Vec2::new(label_width, 24.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let title_text = match kind {
                            SyncBannerKind::SyncingOtherMessages {
                                percentage: Some(pct),
                                ..
                            } => format!("{} ({:.0}%)", STR_SYNC_IN_PROGRESS_BANNER_LABEL, pct),
                            SyncBannerKind::WaitConfirmation { timeout_at } => {
                                let rem = timeout_at
                                    .saturating_duration_since(Instant::now())
                                    .as_secs();
                                if rem > 0 {
                                    format!("Chờ xác nhận điện thoại ({}s)…", rem)
                                } else {
                                    "Chờ xác nhận điện thoại…".to_string()
                                }
                            }
                            _ => kind.title().to_string(),
                        };

                        let tooltip = match kind {
                            SyncBannerKind::Error { message, .. } => message.clone(),
                            SyncBannerKind::Interrupted { description, .. } => description.clone(),
                            SyncBannerKind::Success { message, .. } => message.clone(),
                            SyncBannerKind::SyncingConvListRecent { detail: Some(d) } => d.clone(),
                            SyncBannerKind::SyncingOtherMessages {
                                detail: Some(d), ..
                            } => d.clone(),
                            _ => title_text.clone(),
                        };

                        let label = ui.add_sized(
                            Vec2::new(label_width, 18.0),
                            egui::Label::new(
                                RichText::new(&title_text)
                                    .size(11.5)
                                    .strong()
                                    .color(pal.text_primary),
                            )
                            .truncate(),
                        );
                        label.on_hover_text(tooltip);
                    },
                );

                // 4. CTA / Stop button
                match kind {
                    SyncBannerKind::Suggest => {
                        let btn = egui::Button::new(
                            RichText::new(STR_SYNC_BANNER_COMPACT_SUGGEST_CTA)
                                .size(11.0)
                                .strong(),
                        )
                        .fill(pal.primary)
                        .small();
                        if ui.add(btn).clicked() {
                            triggered_action = Some(SyncBannerAction::StartSuggestSync);
                        }
                    }
                    SyncBannerKind::Error { can_retry, .. } if *can_retry => {
                        let btn = egui::Button::new(
                            RichText::new(STR_SYNC_BANNER_COMPACT_RETRY_CTA)
                                .size(11.0)
                                .strong(),
                        )
                        .fill(pal.primary)
                        .small();
                        if ui.add(btn).clicked() {
                            triggered_action = Some(SyncBannerAction::RetryFailedSync);
                        }
                    }
                    _ if has_stop => {
                        let label = if cancel_requested {
                            "Đang dừng…"
                        } else {
                            "Dừng"
                        };
                        let btn = egui::Button::new(RichText::new(label).size(11.0)).small();
                        if ui.add_enabled(!cancel_requested, btn).clicked() {
                            triggered_action = Some(SyncBannerAction::CancelSync);
                        }
                    }
                    _ => {}
                }

                // 5. Close [x] button with state-specific semantics
                let close_tooltip = match kind {
                    SyncBannerKind::Error { .. } => "Đóng và xóa thông báo lỗi",
                    SyncBannerKind::Suggest => "Ẩn gợi ý đồng bộ",
                    SyncBannerKind::Success { .. } => "Đóng thông báo",
                    _ => "Dừng đồng bộ",
                };
                if ui.small_button("✕").on_hover_text(close_tooltip).clicked() {
                    triggered_action = Some(SyncBannerAction::DismissOrClose);
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
    use std::time::Duration;

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
            Some(SyncBannerKind::Suggest)
        );
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
            Some(SyncBannerKind::Suggest)
        );
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
