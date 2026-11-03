//! Public bundled GIF assets from upstream ed550f6, sent as attachments, never fake server sticker IDs.
use anyhow::{ensure, Result};
/// Local picker category; this value must never be submitted as a Zalo category.
pub(crate) const CATEGORY: u64 = 99901;
/// A built-in item whose immutable bytes can be decoded off the UI thread.
pub(crate) struct Item {
    pub id: u64,
    pub label: &'static str,
    pub png: &'static [u8],
    pub gif: &'static [u8],
}
/// Source assets and labels are retained from upstream ed550f6.
pub(crate) const ITEMS: &[Item] = &[
    Item {
        id: 999001,
        label: "01. Múa bụng cười hả hê",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_01.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_01.gif"),
    },
    Item {
        id: 999002,
        label: "02. Cúi chào tạm biệt (Bye)",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_02.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_02.gif"),
    },
    Item {
        id: 999003,
        label: "03. Xoa tay cười gian xảo",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_03.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_03.gif"),
    },
    Item {
        id: 999004,
        label: "04. Cười tít mắt cute",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_04.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_04.gif"),
    },
    Item {
        id: 999005,
        label: "05. Dậm chân giận dỗi",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_05.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_05.gif"),
    },
    Item {
        id: 999006,
        label: "06. Bắn tim yêu thương",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_06.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_06.gif"),
    },
    Item {
        id: 999007,
        label: "07. Khóc nức nở dưới vòi sen",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_07.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_07.gif"),
    },
    Item {
        id: 999008,
        label: "08. Nằm bẹp thở dài",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_08.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_08.gif"),
    },
    Item {
        id: 999009,
        label: "09. Mắt lấp lánh cầu xin",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_09.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_09.gif"),
    },
    Item {
        id: 999010,
        label: "10. Chống cằm mắt long lanh",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_10.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_10.gif"),
    },
    Item {
        id: 999011,
        label: "11. Ngửa cổ cười nắc nẻ",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_11.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_11.gif"),
    },
    Item {
        id: 999012,
        label: "12. Liếc xéo khó chịu",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_12.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_12.gif"),
    },
    Item {
        id: 999013,
        label: "13. Bò lết lén lút",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_13.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_13.gif"),
    },
    Item {
        id: 999014,
        label: "14. Ngồi trước bát ăn mặt kiêu",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_14.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_14.gif"),
    },
    Item {
        id: 999015,
        label: "15. Nhảy lắc mông rung lắc",
        png: include_bytes!("assets/ami_fat_cat/ami_fat_cat_15.png"),
        gif: include_bytes!("assets/ami_fat_cat/ami_fat_cat_15.gif"),
    },
];
/// Resolve only known local identifiers; arbitrary server IDs are not accepted.
pub(crate) fn find(id: u64) -> Option<&'static Item> {
    ITEMS.iter().find(|item| item.id == id)
}
/// Match the same texture namespace used by the existing sticker picker.
pub(crate) fn thumbnail(key: &str) -> Option<&'static [u8]> {
    key.strip_prefix("stk_99901_")
        .and_then(|s| s.parse().ok())
        .and_then(find)
        .map(|item| item.png)
}
/// Materialize immutable bundled bytes under this session's account directory.
///
/// # Errors
/// Rejects unknown assets, inactive sessions and filesystem failures.
pub(crate) async fn stage(
    session: &crate::session::Session,
    id: u64,
) -> Result<std::path::PathBuf> {
    let item = find(id).ok_or_else(|| anyhow::anyhow!("Nhãn GIF không tồn tại"))?;
    ensure!(
        session.is_active() && !session.account_key().is_empty(),
        "Chưa đăng nhập để gửi GIF"
    );
    let account = session.clone();
    tokio::task::spawn_blocking(move || {
        ensure!(account.is_active(), "Đã đăng xuất; dừng gửi GIF");
        let root = account.history_path().with_extension("bundled-media");
        let path = root.join(format!("ami-fat-cat-{}.gif", item.id));
        crate::storage::atomic_write(&path, item.gif)?;
        Ok(path)
    })
    .await?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_keys_resolve_only_the_embedded_pack() {
        assert_eq!(ITEMS.len(), 15);
        let mut ids = std::collections::HashSet::new();
        for item in ITEMS {
            assert!(ids.insert(item.id));
            assert!(!item.label.is_empty());
            assert!(item.gif.starts_with(b"GIF89a") || item.gif.starts_with(b"GIF87a"));
            assert!(item.png.starts_with(b"\x89PNG\r\n\x1a\n"));
            let image = crate::limits::decode_image(item.png).unwrap();
            assert!(image.width() > 0 && image.height() > 0);
            assert_eq!(
                thumbnail(&format!("stk_{CATEGORY}_{}", item.id)),
                Some(item.png)
            );
        }
        assert!(thumbnail("stk_61900_999001").is_none());
        assert!(thumbnail("stk_99901_../../file").is_none());
        assert!(find(999999).is_none());
    }
    #[tokio::test]
    async fn logged_out_gif_send_never_creates_an_upload_file() {
        let s = crate::session::Session::new(false, false, false).unwrap();
        assert!(stage(&s, 999001).await.is_err());
    }
    #[tokio::test]
    async fn local_category_cannot_leak_to_the_server_sticker_api() {
        let s = crate::session::Session::new(false, false, false).unwrap();
        for group in [false, true] {
            let error = crate::chat::send_sticker_with_client_id(
                &s,
                "fake-recipient",
                999001,
                CATEGORY,
                7,
                group,
                42,
            )
            .await
            .unwrap_err();
            assert!(error.to_string().contains("attachment"));
        }
    }
}
