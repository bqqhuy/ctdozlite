//! Optional HD downscale before a photo upload.
//!
//! Zalo PC sends photos in `PHOTO_RESOLUTION_MODE.HD` by default and only keeps the original
//! file in `ORIGINAL` mode (`_shouldDowngradeOriginal`, `getSendPhotoConfig`). The upload and
//! send requests are the same in both modes; only the uploaded file differs. The fork keeps
//! the original by default, as before, and offers the HD copy as a choice.
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Longest side of an HD copy.
pub const HD_LONG_SIDE: u32 = 2560;
/// Files at or below this size and within `HD_LONG_SIDE` are sent unchanged.
const KEEP_BYTES: u64 = 1_500_000;
/// HD copies older than this are removed; a pending retry rebuilds its copy from the original.
const KEEP_COPIES: std::time::Duration = std::time::Duration::from_secs(7 * 24 * 60 * 60);
/// Total size kept across all accounts' HD copies; the oldest go first.
const MAX_COPY_BYTES: u64 = 500 * 1024 * 1024;
/// Decoding limit for untrusted local files.
const MAX_PIXELS: u64 = 200_000_000;

/// Formats that are re-encoded; GIF keeps its animation and is never touched.
fn reducible(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|e| matches!(e.as_str(), "jpg" | "jpeg" | "png" | "webp" | "bmp"))
}

/// Target size that fits `HD_LONG_SIDE`, or `None` when the image already fits.
pub fn hd_size(width: u32, height: u32) -> Option<(u32, u32)> {
    let long = width.max(height);
    if long <= HD_LONG_SIDE || width == 0 || height == 0 {
        return None;
    }
    let scale = f64::from(HD_LONG_SIDE) / f64::from(long);
    let w = ((f64::from(width) * scale).round() as u32).max(1);
    let h = ((f64::from(height) * scale).round() as u32).max(1);
    Some((w, h))
}

/// Write an HD JPEG copy of `path` into `dir` when that makes it smaller.
/// Returns `None` when the original should be sent as is.
/// # Errors
/// Returns decode and write failures; callers then send the original.
pub fn reduce(path: &Path, dir: &Path) -> Result<Option<PathBuf>> {
    use image::ImageDecoder;
    if !reducible(path) {
        return Ok(None);
    }
    let bytes = std::fs::metadata(path)?.len();
    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    anyhow::ensure!(
        u64::from(width) * u64::from(height) <= MAX_PIXELS,
        "Ảnh quá lớn để xử lý"
    );
    let orientation = decoder.orientation().ok();
    let target = hd_size(width, height);
    if target.is_none() && bytes <= KEEP_BYTES {
        return Ok(None);
    }
    let mut image = image::DynamicImage::from_decoder(decoder)?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    if hd_size(image.width(), image.height()).is_some() {
        image = image.resize(
            HD_LONG_SIDE,
            HD_LONG_SIDE,
            image::imageops::FilterType::Lanczos3,
        );
    }
    let rgb = image.to_rgb8();
    crate::storage::private_dir(dir)?;
    prune(dir, KEEP_COPIES, MAX_COPY_BYTES);
    let mut random = [0u8; 8];
    crate::random::fill(&mut random)?;
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("anh")
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        .take(48)
        .collect::<String>();
    let name = format!(
        "{}-{}.jpg",
        if stem.is_empty() { "anh" } else { &stem },
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let out = dir.join(name);
    let mut encoded = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 90)
        .encode_image(&rgb)
        .context("Không mã hóa được ảnh HD")?;
    if target.is_none() && encoded.len() as u64 >= bytes {
        return Ok(None);
    }
    crate::storage::atomic_write(&out, &encoded)?;
    Ok(Some(out))
}

/// Remove HD copies older than `age`, then the oldest until the folder fits `max_bytes`.
/// Only `.jpg` files this module wrote are touched.
fn prune(dir: &Path, age: std::time::Duration, max_bytes: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let now = std::time::SystemTime::now();
    let mut kept = Vec::new();
    for entry in entries.flatten().take(10_000) {
        let path = entry.path();
        let Some(meta) = entry.metadata().ok().filter(|m| m.is_file()) else {
            continue;
        };
        if path.extension().is_none_or(|e| e != "jpg") {
            continue;
        }
        let modified = meta.modified().unwrap_or(now);
        if now
            .duration_since(modified)
            .is_ok_and(|elapsed| elapsed >= age)
        {
            let _ = std::fs::remove_file(&path);
        } else {
            kept.push((modified, meta.len(), path));
        }
    }
    kept.sort_by_key(|(modified, _, _)| *modified);
    let mut total: u64 = kept.iter().map(|(_, len, _)| *len).sum();
    for (_, len, path) in kept {
        if total <= max_bytes {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// Clean every account's HD copy folder; run once at startup.
pub fn prune_all() {
    let root = crate::storage::root().join("outgoing-photos");
    prune(&root, KEEP_COPIES, MAX_COPY_BYTES);
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten().take(256) {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                prune(&entry.path(), KEEP_COPIES, MAX_COPY_BYTES);
            }
        }
    }
}

/// Folder for one account's HD copies waiting to be sent; the account key is hashed so
/// the path does not reveal it.
pub fn outgoing_dir(account: &str) -> PathBuf {
    let tag = crate::crypto::md5_hex(&format!("outgoing-photos:{account}"));
    crate::storage::root()
        .join("outgoing-photos")
        .join(&tag[..16])
}

/// Remove one account's HD copies (logout); pending sends rebuild them from the originals.
pub fn remove_account_copies(account: &str) {
    let dir = outgoing_dir(account);
    if dir.starts_with(crate::storage::root()) {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hd_size_fits_the_long_side_and_keeps_small_images() {
        assert_eq!(hd_size(4000, 3000), Some((2560, 1920)));
        assert_eq!(hd_size(1000, 6000), Some((427, 2560)));
        assert_eq!(hd_size(2560, 1000), None);
        assert_eq!(hd_size(0, 9000), None);
    }
    #[test]
    fn large_png_becomes_hd_jpeg_and_gif_is_untouched() {
        let root = std::env::temp_dir().join(format!(
            "photo-quality-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("big.png");
        image::RgbImage::from_fn(3000, 1500, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 241) as u8, 7])
        })
        .save(&source)
        .unwrap();
        let out = reduce(&source, &root.join("out")).unwrap().unwrap();
        let reduced = image::open(&out).unwrap();
        assert_eq!((reduced.width(), reduced.height()), (2560, 1280));
        let gif = root.join("a.gif");
        std::fs::write(&gif, b"GIF89a").unwrap();
        assert!(reduce(&gif, &root.join("out")).unwrap().is_none());
        let small = root.join("small.png");
        image::RgbImage::new(10, 10).save(&small).unwrap();
        assert!(reduce(&small, &root.join("out")).unwrap().is_none());
        // Pruning with a zero age removes the leftover copy but keeps other files.
        let keep = root.join("out").join("note.txt");
        std::fs::write(&keep, b"x").unwrap();
        prune(&root.join("out"), std::time::Duration::ZERO, u64::MAX);
        assert!(!out.exists() && keep.exists());
        // The size cap removes the oldest copies first.
        let dir = root.join("cap");
        std::fs::create_dir(&dir).unwrap();
        for (i, name) in ["a.jpg", "b.jpg", "c.jpg"].iter().enumerate() {
            std::fs::write(dir.join(name), vec![0u8; 10]).unwrap();
            let time =
                std::time::SystemTime::now() - std::time::Duration::from_secs(300 - i as u64 * 100);
            std::fs::File::options()
                .write(true)
                .open(dir.join(name))
                .unwrap()
                .set_modified(time)
                .unwrap();
        }
        prune(&dir, std::time::Duration::from_secs(3600), 20);
        assert!(
            !dir.join("a.jpg").exists() && dir.join("b.jpg").exists() && dir.join("c.jpg").exists()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
