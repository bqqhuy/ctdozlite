//! Multi-account profile switcher UI component (WP16-3).
//!
//! Renders a dropdown in the left sidebar showing available profiles with avatars,
//! allowing quick switching between accounts without re-login.

use eframe::egui::{self, Align, Align2, Color32, FontId, Layout, RichText, Rounding, Sense, Ui, Vec2};
use crate::profiles::Profile;

/// Account switcher UI state.
#[allow(dead_code)]
#[derive(Default)]
pub struct AccountSwitcher {
    /// Currently visible (expanded) or collapsed.
    pub expanded: bool,
    /// Profile being hovered.
    pub hover_profile: Option<String>,
}

#[allow(dead_code)]
impl AccountSwitcher {
    /// Render the account switcher in the sidebar header.
    /// Returns the UID of the profile selected, if any.
    pub fn show(&mut self, ui: &mut Ui, active_profile: Option<&Profile>, all_profiles: &[(&str, &Profile)]) -> Option<String> {
        let mut selected_uid = None;

        ui.horizontal(|ui| {
            ui.set_height(44.0);

            // Current profile avatar + name
            if let Some(profile) = active_profile {
                if ui.button(RichText::new(&profile.display_name).size(14.0)).clicked() {
                    self.expanded = !self.expanded;
                }
            } else {
                if ui.button(RichText::new("Select Account").size(14.0)).clicked() {
                    self.expanded = !self.expanded;
                }
            }

            // Dropdown arrow
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let arrow = if self.expanded { "▼" } else { "▶" };
                ui.label(RichText::new(arrow).size(12.0));
            });
        });

        // Expanded dropdown
        if self.expanded && !all_profiles.is_empty() {
            ui.separator();
            let max_height = (all_profiles.len() as f32 * 48.0).min(240.0);

            egui::ScrollArea::vertical()
                .max_height(max_height)
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    for (_, profile) in all_profiles {
                        let is_active = active_profile.map(|p| p.uid == profile.uid).unwrap_or(false);
                        if self.profile_item(ui, profile, is_active) {
                            selected_uid = Some(profile.uid.clone());
                            self.expanded = false;
                        }
                    }
                });
            ui.separator();
        }

        selected_uid
    }

    /// Render a single profile in the dropdown.
    fn profile_item(&mut self, ui: &mut Ui, profile: &Profile, is_active: bool) -> bool {
        let item_height = 44.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), item_height), Sense::click());

        let bg_color = if is_active {
            Color32::from_rgb(38, 99, 255) // Primary blue
        } else if response.hovered() {
            Color32::from_rgb(245, 245, 245) // Light gray
        } else {
            Color32::TRANSPARENT
        };

        ui.painter().rect_filled(rect, Rounding::same(4.0), bg_color);

        let text_color = if is_active {
            Color32::WHITE
        } else {
            Color32::from_rgb(0, 0, 0)
        };

        // Avatar placeholder (circle)
        let avatar_size = 32.0;
        let avatar_rect = egui::Rect::from_min_size(
            rect.min + Vec2::new(6.0, (item_height - avatar_size) / 2.0),
            Vec2::new(avatar_size, avatar_size),
        );
        ui.painter().circle_filled(
            avatar_rect.center(),
            avatar_size / 2.0,
            Color32::from_rgb(200, 200, 200),
        );

        // Name and phone using painter
        let name_pos = rect.min + Vec2::new(50.0, 6.0);
        ui.painter().text(
            name_pos,
            Align2::LEFT_TOP,
            &profile.display_name,
            FontId::proportional(13.0),
            text_color,
        );

        if let Some(phone) = &profile.phone {
            let phone_pos = rect.min + Vec2::new(50.0, 24.0);
            ui.painter().text(
                phone_pos,
                Align2::LEFT_TOP,
                phone,
                FontId::proportional(11.0),
                Color32::from_rgb(128, 128, 128),
            );
        }

        // Checkmark for active
        if is_active {
            ui.painter().text(
                rect.right_top() + Vec2::new(-12.0, 6.0),
                Align2::CENTER_TOP,
                "✓",
                FontId::proportional(16.0),
                Color32::WHITE,
            );
        }

        response.clicked()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switcher_starts_collapsed() {
        let switcher = AccountSwitcher::default();
        assert!(!switcher.expanded);
    }

    #[test]
    fn switcher_toggles_expanded() {
        let switcher = AccountSwitcher {
            expanded: true,
            ..Default::default()
        };
        assert!(switcher.expanded);
    }
}
