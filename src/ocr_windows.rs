//! Local OCR configuration and image preprocessing pipeline.
//!
//! Provides E2-verified Zalo PC 26.11.10 OCR parameters, bounded image ingestion,
//! proportional downscaling to `max_pixel_size = 1280`, and capability status reporting.

use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};

/// Default maximum dimension (width or height) for OCR image preprocessing.
pub const DEFAULT_MAX_PIXEL_SIZE: u32 = 1280;
/// Maximum allowed raw image byte payload (16 MiB).
pub const MAX_IMAGE_BYTES: usize = 16 * 1024 * 1024;
/// Default idle time-to-live for OCR runtime sessions in seconds.
pub const DEFAULT_IDLE_TTL_SECS: u64 = 60;

/// Detection and recognition thresholds verified from Zalo PC 26.11.10 OCR specifications.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OcrThresholds {
    /// Binarization threshold for DB text detector.
    pub det_db_thresh: f32,
    /// Box threshold for DB text detector polygon formation.
    pub det_db_box_thresh: f32,
    /// Text recognition confidence score threshold.
    pub rec_score_thresh: f32,
}

impl Default for OcrThresholds {
    fn default() -> Self {
        Self {
            det_db_thresh: 0.3,
            det_db_box_thresh: 0.6,
            rec_score_thresh: 0.5,
        }
    }
}

/// Configuration settings for the OCR preprocessing and engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Maximum pixel dimension (width or height) before proportional downscaling.
    pub max_pixel_size: u32,
    /// Whether to detect and normalize text line orientation.
    pub textline_orientation: bool,
    /// Model detection and recognition thresholds.
    pub thresholds: OcrThresholds,
    /// Idle session time-to-live in seconds before releasing OCR runtime resources.
    pub idle_ttl_secs: u64,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            max_pixel_size: DEFAULT_MAX_PIXEL_SIZE,
            textline_orientation: true,
            thresholds: OcrThresholds::default(),
            idle_ttl_secs: DEFAULT_IDLE_TTL_SECS,
        }
    }
}

/// Preprocessed image ready for OCR inference.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedOcrImage {
    /// Resulting width in pixels after preprocessing.
    pub width: u32,
    /// Resulting height in pixels after preprocessing.
    pub height: u32,
    /// Original width in pixels prior to downscaling.
    pub original_width: u32,
    /// Original height in pixels prior to downscaling.
    pub original_height: u32,
    /// Raw RGB8 byte buffer.
    pub rgb_bytes: Vec<u8>,
    /// Estimated contrast ratio (0.0 to 1.0) for diagnostic heuristics.
    pub estimated_contrast: f32,
}

/// Truthful status of OCR capabilities available in this runtime environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OcrCapabilityStatus {
    /// Windows Media OCR or native backend is fully linked and ready.
    Ready,
    /// Preprocessing and bounding pipeline is active; full WinRT OCR runtime is unlinked.
    ExperimentalPreprocessOnly,
}

/// Returns the current OCR capability status.
pub fn get_ocr_capability_status() -> OcrCapabilityStatus {
    // WinRT Windows.Media.Ocr is unlinked; always return ExperimentalPreprocessOnly
    OcrCapabilityStatus::ExperimentalPreprocessOnly
}

/// Ingest raw image bytes, validate bounds, decode, downscale proportionally if needed,
/// and compute text-region heuristics.
pub fn prepare_image_for_ocr(image_bytes: &[u8], config: &OcrConfig) -> Result<PreparedOcrImage> {
    ensure!(!image_bytes.is_empty(), "Dữ liệu hình ảnh trống");
    ensure!(
        image_bytes.len() <= MAX_IMAGE_BYTES,
        "Kích thước hình ảnh vượt quá giới hạn 16 MiB ({len} bytes)",
        len = image_bytes.len()
    );

    let img = match image::load_from_memory(image_bytes) {
        Ok(decoded) => decoded,
        Err(e) => bail!("Không thể giải mã hình ảnh: {e}"),
    };

    let original_width = img.width();
    let original_height = img.height();

    ensure!(
        original_width > 0 && original_height > 0,
        "Kích thước hình ảnh không hợp lệ (0x0)"
    );

    let max_dim = original_width.max(original_height);
    let target_max = config.max_pixel_size.max(64);

    let processed = if max_dim > target_max {
        let scale = target_max as f64 / max_dim as f64;
        let new_w = ((original_width as f64 * scale).round() as u32).max(1);
        let new_h = ((original_height as f64 * scale).round() as u32).max(1);
        img.resize_exact(new_w, new_h, image::imageops::FilterType::Triangle)
    } else {
        img
    };

    let rgb_img = processed.to_rgb8();
    let width = rgb_img.width();
    let height = rgb_img.height();
    let raw = rgb_img.into_raw();

    // Compute basic luminance contrast heuristic
    let mut min_luma = 255u8;
    let mut max_luma = 0u8;
    let (chunks, _) = raw.as_chunks::<3>();
    for chunk in chunks {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;
        // Standard Rec. 601 luma
        let luma = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
        if luma < min_luma {
            min_luma = luma;
        }
        if luma > max_luma {
            max_luma = luma;
        }
    }
    let estimated_contrast = if max_luma > min_luma {
        (max_luma - min_luma) as f32 / 255.0
    } else {
        0.0
    };

    Ok(PreparedOcrImage {
        width,
        height,
        original_width,
        original_height,
        rgb_bytes: raw,
        estimated_contrast,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_config_defaults_and_overrides() {
        let default_cfg = OcrConfig::default();
        assert_eq!(default_cfg.max_pixel_size, 1280);
        assert!(default_cfg.textline_orientation);
        assert_eq!(default_cfg.thresholds.det_db_thresh, 0.3);
        assert_eq!(default_cfg.thresholds.det_db_box_thresh, 0.6);
        assert_eq!(default_cfg.thresholds.rec_score_thresh, 0.5);
        assert_eq!(default_cfg.idle_ttl_secs, 60);

        let custom_cfg = OcrConfig {
            max_pixel_size: 1600,
            textline_orientation: false,
            thresholds: OcrThresholds {
                det_db_thresh: 0.4,
                det_db_box_thresh: 0.7,
                rec_score_thresh: 0.6,
            },
            idle_ttl_secs: 120,
        };
        assert_eq!(custom_cfg.max_pixel_size, 1600);
        assert!(!custom_cfg.textline_orientation);
        assert_eq!(custom_cfg.thresholds.det_db_thresh, 0.4);
        assert_eq!(custom_cfg.thresholds.det_db_box_thresh, 0.7);
        assert_eq!(custom_cfg.thresholds.rec_score_thresh, 0.6);
        assert_eq!(custom_cfg.idle_ttl_secs, 120);
    }

    #[test]
    fn test_prepare_image_downscales_to_max_pixel_size() {
        // Create an uncompressed 2000 x 1000 PNG image with large black and white blocks
        let img = image::RgbImage::from_fn(2000, 1000, |x, y| {
            if (x / 100 + y / 100) % 2 == 0 {
                image::Rgb([255, 255, 255])
            } else {
                image::Rgb([0, 0, 0])
            }
        });
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();

        let cfg = OcrConfig::default();
        let prepared = prepare_image_for_ocr(&png_bytes, &cfg).unwrap();

        assert_eq!(prepared.original_width, 2000);
        assert_eq!(prepared.original_height, 1000);
        assert_eq!(prepared.width, 1280);
        assert_eq!(prepared.height, 640);
        assert_eq!(prepared.rgb_bytes.len(), (1280 * 640 * 3) as usize);
        assert!(prepared.estimated_contrast > 0.8);
    }

    #[test]
    fn test_prepare_image_keeps_smaller_dimensions() {
        let img = image::RgbImage::from_pixel(300, 200, image::Rgb([128, 128, 128]));
        let mut png_bytes = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();

        let cfg = OcrConfig::default();
        let prepared = prepare_image_for_ocr(&png_bytes, &cfg).unwrap();

        assert_eq!(prepared.original_width, 300);
        assert_eq!(prepared.original_height, 200);
        assert_eq!(prepared.width, 300);
        assert_eq!(prepared.height, 200);
    }

    #[test]
    fn test_prepare_image_rejects_empty_and_malformed() {
        let cfg = OcrConfig::default();
        assert!(prepare_image_for_ocr(&[], &cfg).is_err());
        assert!(prepare_image_for_ocr(&[1, 2, 3, 4, 5], &cfg).is_err());
    }

    #[test]
    fn test_ocr_capability_status() {
        let status = get_ocr_capability_status();
        assert_eq!(status, OcrCapabilityStatus::ExperimentalPreprocessOnly);
    }
}
