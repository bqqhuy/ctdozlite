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
/// Decode a bundled animated GIF sticker into a horizontally stitched sprite sheet.
pub fn decode_gif_sprite(id: u64) -> Option<crate::gui::DecodedSpriteData> {
    use image::codecs::gif::GifDecoder;
    use image::AnimationDecoder;

    let item = find(id)?;
    let decoder = GifDecoder::new(std::io::Cursor::new(item.gif)).ok()?;
    let frames = decoder.into_frames().collect_frames().ok()?;
    if frames.is_empty() {
        return None;
    }

    let frame_count = frames.len();
    let orig_width = frames[0].buffer().width() as usize;
    let orig_height = frames[0].buffer().height() as usize;
    if orig_width == 0 || orig_height == 0 {
        return None;
    }

    // Target preview scale: Zalo sticker preview size is ~116-130px.
    // If original width > 160 or total_width > 4096, scale down each frame to 160px width
    // to strictly preserve GPU max_texture_side <= 8192 and conserve 90% VRAM/RAM.
    let (width, height, resized_frames) = if orig_width > 160 || orig_width * frame_count > 4096 {
        let target_w = 160usize;
        let target_h = ((orig_height * target_w) / orig_width).max(1);
        let mut resized = Vec::with_capacity(frame_count);
        for frame in &frames {
            let resized_buf = image::imageops::resize(
                frame.buffer(),
                target_w as u32,
                target_h as u32,
                image::imageops::FilterType::Triangle,
            );
            resized.push(resized_buf);
        }
        (target_w, target_h, Some(resized))
    } else {
        (orig_width, orig_height, None)
    };

    let total_width = width * frame_count;
    if total_width > 8192 || height > 8192 {
        return None;
    }

    let mut stitched_pixels = vec![0u8; total_width * height * 4];

    for (f, frame) in frames.iter().enumerate() {
        let (frame_w, frame_h, raw, buffer_stride) = if let Some(ref r) = resized_frames {
            let buf = &r[f];
            (
                buf.width() as usize,
                buf.height() as usize,
                buf.as_raw().as_slice(),
                buf.width() as usize * 4,
            )
        } else {
            let buf = frame.buffer();
            (
                (buf.width() as usize).min(width),
                (buf.height() as usize).min(height),
                buf.as_raw().as_slice(),
                buf.width() as usize * 4,
            )
        };

        for y in 0..frame_h {
            let src_start = y * buffer_stride;
            let src_end = src_start + frame_w * 4;
            let dst_start = (y * total_width + f * width) * 4;
            let dst_end = dst_start + frame_w * 4;
            stitched_pixels[dst_start..dst_end].copy_from_slice(&raw[src_start..src_end]);
        }
    }

    let delays: Vec<std::time::Duration> = frames
        .iter()
        .map(|f| {
            let d: std::time::Duration = f.delay().into();
            d.max(std::time::Duration::from_millis(20))
        })
        .collect();

    let image =
        eframe::egui::ColorImage::from_rgba_unmultiplied([total_width, height], &stitched_pixels);

    Some(crate::gui::DecodedSpriteData {
        image,
        frames: frame_count as u16,
        delays,
    })
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
    #[test]
    fn test_decode_gif_sprite() {
        assert!(decode_gif_sprite(999999).is_none());
        for item in ITEMS {
            let sprite = decode_gif_sprite(item.id).expect("Sprite decode failed");
            assert!(sprite.frames > 0);
            assert_eq!(sprite.delays.len(), sprite.frames as usize);
            for d in &sprite.delays {
                assert!(*d >= std::time::Duration::from_millis(20));
            }
            assert_eq!(sprite.image.size[0] % sprite.frames as usize, 0);
            let frame_width = sprite.image.size[0] / sprite.frames as usize;
            assert!(frame_width > 0);
            assert!(sprite.image.size[1] > 0);
            assert!(sprite.image.size[0] <= 8192, "Sprite width {} exceeds 8192", sprite.image.size[0]);
            assert!(sprite.image.size[1] <= 8192, "Sprite height {} exceeds 8192", sprite.image.size[1]);
        }
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
