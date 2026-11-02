//! Bounded NV12 camera normalization and capture cadence, independent of the driver.
use anyhow::{ensure, Result};

pub(crate) const OUTPUT_WIDTH: usize = 320;
pub(crate) const OUTPUT_HEIGHT: usize = 240;
pub(crate) const OUTPUT_FPS: i64 = 15;

/// Prefer a color camera over IR/Hello/depth devices in automatic selection.
pub(crate) fn camera_preference(name: &str) -> u8 {
    u8::from(name.split(|c: char| !c.is_alphanumeric()).any(|word| {
        ["ir", "infrared", "hello", "depth"]
            .iter()
            .any(|special| word.eq_ignore_ascii_case(special))
    }))
}

/// Validate an NV12 buffer before accessing its padded Y/UV planes.
pub(crate) fn validate_layout(
    width: usize,
    height: usize,
    stride: usize,
    len: usize,
) -> Result<()> {
    ensure!(
        width > 0
            && height > 0
            && width.is_multiple_of(2)
            && height.is_multiple_of(2)
            && width <= 1920
            && height <= 1920
            && width * height <= 1920 * 1080,
        "Kích thước camera không được hỗ trợ: {width}x{height}"
    );
    ensure!(
        stride >= width && stride <= 16_384,
        "Stride camera không hợp lệ"
    );
    let size = stride
        .checked_mul(height + height / 2)
        .ok_or_else(|| anyhow::anyhow!("NV12 size overflow"))?;
    ensure!(size <= len, "Khung camera NV12 không đầy đủ");
    Ok(())
}

/// Pack/resize a bounded NV12 frame to 320x240 without stretching its aspect ratio.
/// Letterbox values are limited-range black; source padding is never copied.
/// # Errors
/// Returns an error for invalid dimensions, stride or truncated planes.
pub(crate) fn normalize_nv12(
    src: &[u8],
    width: usize,
    height: usize,
    stride: usize,
) -> Result<Vec<u8>> {
    validate_layout(width, height, stride, src.len())?;
    let (tw, th) = (OUTPUT_WIDTH, OUTPUT_HEIGHT);
    let (dw, dh) = if width * th > height * tw {
        (tw, ((height * tw / width) & !1).max(2))
    } else {
        (((width * th / height) & !1).max(2), th)
    };
    let (left, top) = (((tw - dw) / 2) & !1, ((th - dh) / 2) & !1);
    let mut out = vec![16; tw * th * 3 / 2];
    out[tw * th..].fill(128);
    for y in 0..dh {
        let sy = y * height / dh;
        for x in 0..dw {
            out[(top + y) * tw + left + x] = src[sy * stride + x * width / dw];
        }
    }
    let uv = stride * height;
    for y in 0..dh / 2 {
        let sy = y * (height / 2) / (dh / 2);
        for x in 0..dw / 2 {
            let sx = x * (width / 2) / (dw / 2);
            let dst = tw * th + (top / 2 + y) * tw + left + x * 2;
            out[dst..dst + 2]
                .copy_from_slice(&src[uv + sy * stride + sx * 2..uv + sy * stride + sx * 2 + 2]);
        }
    }
    Ok(out)
}

/// Select at most fifteen source frames per second using Media Foundation's
/// 100-nanosecond timestamps, independent of read/encoding thread scheduling.
#[derive(Default)]
pub(crate) struct CaptureCadence {
    origin: Option<i64>,
    previous: i64,
    slot: i128,
}
impl CaptureCadence {
    pub(crate) fn accepts(&mut self, timestamp: i64) -> bool {
        let Some(origin) = self.origin.filter(|_| timestamp >= self.previous) else {
            self.origin = Some(timestamp);
            self.previous = timestamp;
            self.slot = 0;
            return true;
        };
        self.previous = timestamp;
        let slot = ((timestamp as i128 - origin as i128) * OUTPUT_FPS as i128 + OUTPUT_FPS as i128)
            / 10_000_000;
        if slot <= self.slot {
            return false;
        }
        self.slot = slot;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn automatic_selection_prefers_color_without_matching_ir_inside_other_words() {
        assert_eq!(camera_preference("Huawei Color Camera"), 0);
        assert_eq!(camera_preference("Huawei IR Camera"), 1);
        assert_eq!(camera_preference("Windows Hello (Infrared)"), 1);
        assert_eq!(camera_preference("First Camera"), 0);
    }
    #[test]
    fn padded_frame_is_packed_without_padding_bytes() {
        let (w, h, stride) = (320, 240, 336);
        let mut src = vec![255; stride * h * 3 / 2];
        for y in 0..h {
            src[y * stride..y * stride + w].fill(100);
        }
        for y in 0..h / 2 {
            src[stride * h + y * stride..stride * h + y * stride + w].fill(128);
        }
        let out = normalize_nv12(&src, w, h, stride).unwrap();
        assert_eq!(out.len(), w * h * 3 / 2);
        assert!(out[..w * h].iter().all(|&b| b == 100));
        assert!(out[w * h..].iter().all(|&b| b == 128));
    }
    #[test]
    fn widescreen_camera_is_letterboxed_instead_of_stretched() {
        let (w, h) = (1280, 720);
        let mut src = vec![90; w * h * 3 / 2];
        src[w * h..].fill(160);
        let out = normalize_nv12(&src, w, h, w).unwrap();
        assert!(out[..320 * 30].iter().all(|&b| b == 16));
        assert_eq!(out[320 * 30], 90);
        assert_eq!(out[320 * 209 + 319], 90);
        assert!(out[320 * 210..320 * 240].iter().all(|&b| b == 16));
        assert_eq!(out[320 * 240 + 320 * 15], 160);
    }
    #[test]
    fn truncated_or_invalid_camera_frames_are_rejected() {
        for (w, h, s, n) in [
            (0, 240, 320, 100),
            (320, 241, 320, 200000),
            (320, 240, 319, 200000),
            (320, 240, usize::MAX, 200000),
            (4000, 2000, 4000, 0),
            (320, 240, 320, 100),
        ] {
            assert!(validate_layout(w, h, s, n).is_err());
        }
    }
    #[test]
    fn thirty_and_sixty_fps_sources_are_throttled_to_fifteen() {
        for fps in [30, 60] {
            let mut cadence = CaptureCadence::default();
            let count = (0..fps)
                .filter(|frame| cadence.accepts(frame * 10_000_000 / fps))
                .count();
            assert_eq!(count, 15);
            assert!(cadence.accepts(-1), "Driver timestamp restart must recover");
        }
    }
}
