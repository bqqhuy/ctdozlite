//! Quick Messages & Shortcut Expansions (Zalo Parity B07).
//!
//! Allows users to define predefined quick messages (e.g., `/dc` -> address, `/stk` -> bank details)
//! that expand automatically when typed in the composer.

use serde::{Deserialize, Serialize};

/// A predefined quick message item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickMessage {
    pub shortcut: String,
    pub title: String,
    pub text: String,
}

/// Account-local user-defined quick messages.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickMessageStore {
    pub items: Vec<QuickMessage>,
}

impl QuickMessageStore {
    /// Search for quick messages matching prefix (e.g. "/" or "/d").
    pub fn search(&self, query: &str) -> Vec<&QuickMessage> {
        let q = query.to_lowercase();
        self.items
            .iter()
            .filter(|it| {
                it.shortcut.to_lowercase().contains(&q) || it.title.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Match an exact shortcut and expand it.
    pub fn expand(&self, text: &str) -> Option<&str> {
        let trimmed = text.trim();
        self.items
            .iter()
            .find(|it| it.shortcut.eq_ignore_ascii_case(trimmed))
            .map(|it| it.text.as_str())
    }

    /// Add or update a quick message.
    pub fn upsert(&mut self, shortcut: String, title: String, text: String) {
        if let Some(pos) = self
            .items
            .iter()
            .position(|it| it.shortcut.eq_ignore_ascii_case(&shortcut))
        {
            self.items[pos] = QuickMessage {
                shortcut,
                title,
                text,
            };
        } else {
            self.items.push(QuickMessage {
                shortcut,
                title,
                text,
            });
        }
    }

    /// Remove a quick message by shortcut.
    pub fn remove(&mut self, shortcut: &str) -> bool {
        if let Some(pos) = self
            .items
            .iter()
            .position(|it| it.shortcut.eq_ignore_ascii_case(shortcut))
        {
            self.items.remove(pos);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quick_message_expansion() {
        let store = QuickMessageStore {
            items: vec![
                QuickMessage {
                    shortcut: "/dc".into(),
                    title: "Địa chỉ".into(),
                    text: "Địa chỉ nhận hàng: Hà Nội, Việt Nam".into(),
                },
                QuickMessage {
                    shortcut: "/chao".into(),
                    title: "Chào hỏi".into(),
                    text: "Xin chào".into(),
                },
            ],
        };
        assert_eq!(
            store.expand("/dc"),
            Some("Địa chỉ nhận hàng: Hà Nội, Việt Nam")
        );
        assert_eq!(store.expand("/none"), None);

        let matches = store.search("chao");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].shortcut, "/chao");
    }

    #[test]
    fn test_quick_message_upsert_and_remove() {
        let mut store = QuickMessageStore::default();
        store.upsert(
            "/test".into(),
            "Thử nghiệm".into(),
            "Nội dung thử nghiệm".into(),
        );
        assert_eq!(store.expand("/test"), Some("Nội dung thử nghiệm"));

        assert!(store.remove("/test"));
        assert_eq!(store.expand("/test"), None);
    }
}
