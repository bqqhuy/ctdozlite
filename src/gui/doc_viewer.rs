//! Document preview modal for plain text, JSON, and log files.
//!
//! Provides a bounded 1 MiB text preview (`Xem trước văn bản (tối đa 1 MiB)`).
//! Note: this is a bounded plain-text viewer, not a binary PDF or Microsoft Office layout engine.

use eframe::egui;

#[derive(Clone)]
pub struct DocViewerModal {
    pub content: String,
    pub file_name: String,
    pub search_query: String,
    pub search_results: Vec<usize>, // Line numbers where query is found
    pub search_index: usize,        // Current position in search_results
}

impl DocViewerModal {
    pub fn new(content: String, file_name: String) -> Self {
        Self {
            content,
            file_name,
            search_query: String::new(),
            search_results: Vec::new(),
            search_index: 0,
        }
    }

    /// Update search results based on current query.
    fn update_search(&mut self) {
        self.search_results.clear();
        self.search_index = 0;
        if self.search_query.is_empty() {
            return;
        }
        let query_lower = self.search_query.to_lowercase();
        for (line_num, line) in self.content.lines().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                self.search_results.push(line_num);
            }
        }
    }

    /// Get the next search result, wrapping around if needed.
    fn next_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        self.search_index = (self.search_index + 1) % self.search_results.len();
    }

    /// Get the previous search result, wrapping around if needed.
    fn prev_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        self.search_index = if self.search_index == 0 {
            self.search_results.len() - 1
        } else {
            self.search_index - 1
        };
    }

    /// Get the current highlighted line number (0-indexed).
    fn current_highlight_line(&self) -> Option<usize> {
        if self.search_results.is_empty() {
            None
        } else {
            Some(self.search_results[self.search_index])
        }
    }
}

impl super::ZaloGuiApp {
    /// Render document viewer modal for text/code/log files (WP15-4 Task 4c).
    pub(super) fn render_doc_viewer_modal(
        &mut self,
        ctx: &egui::Context,
        pal: &super::ThemePalette,
    ) {
        // Escape to close
        let should_close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        if should_close {
            self.doc_viewer = None;
            return;
        }

        let Some(ref mut viewer) = self.doc_viewer else {
            return;
        };

        // Ctrl+F to focus search
        let ctrl = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let focus_search = ctrl && ctx.input(|i| i.key_pressed(egui::Key::F));

        let file_name = viewer.file_name.clone();
        let mut close_clicked = false;

        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(pal.bg_app))
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    // Top bar: filename + close button
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "📄 Xem trước văn bản (tối đa 1 MiB): {file_name}"
                            ))
                            .strong()
                            .color(pal.text_primary),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕ Đóng").clicked() {
                                close_clicked = true;
                            }
                        });
                    });

                    ui.separator();

                    // Search bar
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        let search_response = ui.text_edit_singleline(&mut viewer.search_query);
                        if focus_search {
                            search_response.request_focus();
                        }
                        if search_response.changed() {
                            viewer.update_search();
                        }
                        if !viewer.search_results.is_empty() {
                            ui.label(format!(
                                "{} / {}",
                                viewer.search_index + 1,
                                viewer.search_results.len()
                            ));
                            if ui.button("▲").clicked() {
                                viewer.prev_search_result();
                            }
                            if ui.button("▼").clicked() {
                                viewer.next_search_result();
                            }
                        }
                    });

                    ui.separator();

                    // Scrollable content with line numbers
                    egui::ScrollArea::vertical()
                        .id_salt("doc_viewer_scroll")
                        .show(ui, |ui| {
                            let highlight_line = viewer.current_highlight_line();
                            let font_id = egui::FontId::monospace(12.0);

                            for (line_num, line) in viewer.content.lines().enumerate() {
                                ui.horizontal(|ui| {
                                    // Line number gutter
                                    ui.label(
                                        egui::RichText::new(format!("{:4} ", line_num + 1))
                                            .font(font_id.clone())
                                            .color(pal.text_secondary),
                                    );

                                    // Line content with highlight
                                    let bg_highlight = if Some(line_num) == highlight_line {
                                        pal.bg_selected
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    let label = egui::Label::new(
                                        egui::RichText::new(line)
                                            .font(font_id.clone())
                                            .color(pal.text_primary)
                                            .background_color(bg_highlight),
                                    )
                                    .selectable(true);
                                    ui.add(label);
                                });
                            }
                        });
                });
            });

        if close_clicked {
            self.doc_viewer = None;
        }
    }
}
