//! UI-only translation catalog. Message bodies, names and filenames are never translated.
use std::{collections::BTreeMap, sync::OnceLock};
thread_local! {static ENGLISH:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
/// Apply the saved locale on the UI thread; account data is unaffected.
pub fn set_language(language: &str) {
    ENGLISH.set(language == "English");
}
fn catalog() -> &'static BTreeMap<String, String> {
    static CATALOG: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("i18n/en.json"))
            .expect("bundled translation catalog must be valid")
    })
}
/// Translate application-owned labels only; unknown labels retain their source wording.
pub fn tr(text: &str) -> &str {
    if ENGLISH.get() {
        catalog().get(text).map(String::as_str).unwrap_or(text)
    } else {
        text
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_contains_core_search_library_and_settings_labels() {
        for key in [
            "Cài đặt",
            "Ngôn ngữ",
            "Từ ngày",
            "Đến ngày",
            "Tất cả người gửi",
            "Nhắc hẹn",
            "Ảnh / Video",
            "Tìm kiếm",
        ] {
            assert!(catalog()
                .get(key)
                .is_some_and(|v| !v.is_empty() && v != key));
        }
    }
}
