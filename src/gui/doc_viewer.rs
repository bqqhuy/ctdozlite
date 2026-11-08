//! Document preview modal for plain text, JSON, and log files.

use eframe::egui::{self, Id, RichText};

#[derive(Clone)]
pub struct DocViewerModal {
    pub content: String,
    pub file_name: String,
    pub scroll: f32,
    pub search_query: String,
    pub search_results: Vec<usize>, // Line numbers where query is found
    pub search_index: usize,         // Current position in search_results
}

impl DocViewerModal {
    pub fn new(content: String, file_name: String) -> Self {
        Self {
            content,
            file_name,
            scroll: 0.0,
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
    pub(super) fn render_doc_viewer_modal(
        &mut self,
        ctx: &egui::Context,
        pal: &super::ThemePalette,
    ) {
        // Placeholder: Document viewer not yet integrated.
        // This stub allows the modal type to be available for future v0.016+ implementation.
        // - Would render a full-screen modal with searchable text content
        // - Line numbers in left gutter
        // - Search bar at top
        // - Scrollable content area with monospace font
        // - Hotkeys: Ctrl+F for search, Ctrl+S to save to downloads folder
    }
}
