//! Limits apply before allocating/decoding untrusted network data.
use std::io::{Cursor, Read};
/// Background avatars/stickers leave two network slots for visible photos.
pub static BACKGROUND_IMAGE_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
pub static IMAGE_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

/// At most two files stream concurrently; additional files await a permit.
pub static UPLOAD_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
pub const IMAGE_BYTES: usize = 16 * 1024 * 1024;
pub const INFLATED_BYTES: u64 = 8 * 1024 * 1024;

pub async fn response_bytes(
    mut response: reqwest::Response,
    limit: usize,
) -> anyhow::Result<Vec<u8>> {
    if let Some(size) = response.content_length() {
        anyhow::ensure!(size <= limit as u64, "Response exceeds size limit");
    }
    let mut output = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        anyhow::ensure!(
            chunk.len() <= limit.saturating_sub(output.len()),
            "Response exceeds size limit"
        );
        output.extend_from_slice(&chunk);
    }
    Ok(output)
}

pub fn decode_image(bytes: &[u8]) -> image::ImageResult<image::DynamicImage> {
    if bytes.len() > IMAGE_BYTES {
        return Err(image::ImageError::IoError(std::io::Error::other(
            "Image exceeds size limit",
        )));
    }
    let (width, height) = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()?;
    if (width as u64) * (height as u64) > 16 * 1024 * 1024 {
        return Err(image::ImageError::Limits(
            image::error::LimitError::from_kind(image::error::LimitErrorKind::DimensionError),
        ));
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode()
}

/// Bound file reads before allocation, including local clipboard/cache previews.
/// # Errors
/// Returns an I/O/size error and never reads more than limit+1 bytes.
pub fn read_file(path: &std::path::Path, limit: usize) -> anyhow::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    anyhow::ensure!(
        file.metadata()?.len() <= limit as u64,
        "Tệp vượt giới hạn dung lượng cho phép"
    );
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= limit,
        "Tệp vượt giới hạn dung lượng cho phép"
    );
    Ok(bytes)
}

/// Build a bounded GUI preview. Saved/downloaded originals are not modified.
pub fn preview_image(bytes: &[u8], key: &str) -> anyhow::Result<image::RgbaImage> {
    let image = decode_image(bytes)?;
    let maximum = if key.starts_with("full_") {
        2048
    } else if key.starts_with("ava_") {
        256
    } else {
        512
    };
    let mut rgba = if image.width() <= maximum && image.height() <= maximum {
        image.to_rgba8()
    } else {
        image.thumbnail(maximum, maximum).to_rgba8()
    };
    if key.starts_with("ava_") {
        let side = rgba.width().min(rgba.height());
        anyhow::ensure!(side > 0, "Empty avatar");
        let square = image::imageops::crop_imm(
            &rgba,
            (rgba.width() - side) / 2,
            (rgba.height() - side) / 2,
            side,
            side,
        )
        .to_image();
        rgba = image::imageops::resize(&square, 128, 128, image::imageops::FilterType::Triangle);
        let center = 63.5f32;
        let radius = 64f32;
        for (x, y, pixel) in rgba.enumerate_pixels_mut() {
            let distance = ((x as f32 - center).powi(2) + (y as f32 - center).powi(2)).sqrt();
            let alpha = (radius + 0.8 - distance).clamp(0.0, 1.0);
            pixel[3] = (pixel[3] as f32 * alpha) as u8;
        }
    }
    Ok(rgba)
}

pub fn read_inflated(reader: impl Read) -> anyhow::Result<Vec<u8>> {
    let mut output = Vec::new();
    reader.take(INFLATED_BYTES + 1).read_to_end(&mut output)?;
    anyhow::ensure!(
        output.len() as u64 <= INFLATED_BYTES,
        "Decompressed payload exceeds size limit"
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compressed_expansion_is_bounded() {
        use std::io::Write;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        std::io::copy(
            &mut std::io::repeat(0).take(INFLATED_BYTES + 1),
            &mut encoder,
        )
        .unwrap();
        encoder.flush().unwrap();
        let bytes = encoder.finish().unwrap();
        assert!(read_inflated(flate2::read::GzDecoder::new(bytes.as_slice())).is_err());
        assert_eq!(read_inflated(Cursor::new(b"small")).unwrap(), b"small");
    }
    #[test]
    fn image_limits_reject_oversized_input() {
        assert!(decode_image(&vec![0; IMAGE_BYTES + 1]).is_err());
    }
    #[tokio::test]
    async fn chunked_response_cannot_bypass_limit() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await;
            socket.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n5\r\nworld\r\n0\r\n\r\n").await.unwrap();
        });
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap();
        assert!(response_bytes(response, 8).await.is_err());
        server.await.unwrap();
    }
}

/// A bounded network slot; background work leaves capacity for visible media.
pub struct ImageNetworkPermit {
    _background: Option<tokio::sync::SemaphorePermit<'static>>,
    _active: tokio::sync::SemaphorePermit<'static>,
}
/// Acquire background capacity before a shared slot so waiting background jobs
/// cannot occupy the capacity reserved for foreground photos.
pub async fn image_network_permit(
    background: bool,
) -> Result<ImageNetworkPermit, tokio::sync::AcquireError> {
    let permit = if background {
        Some(BACKGROUND_IMAGE_JOBS.acquire().await?)
    } else {
        None
    };
    Ok(ImageNetworkPermit {
        _background: permit,
        _active: IMAGE_JOBS.acquire().await?,
    })
}
#[cfg(test)]
mod image_priority_tests {
    use super::*;
    #[tokio::test]
    async fn slow_background_requests_leave_foreground_capacity() {
        let a = image_network_permit(true).await.unwrap();
        let b = image_network_permit(true).await.unwrap();
        let waiting = tokio::spawn(async { image_network_permit(true).await.unwrap() });
        let foreground = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            image_network_permit(false),
        )
        .await
        .expect("visible photo must not wait for background")
        .unwrap();
        // Other image tests may concurrently use the global foreground slots.
        // The contract is admission of this foreground request while background waits.
        assert!(!waiting.is_finished());
        drop(foreground);
        drop(a);
        drop(b);
        drop(waiting.await.unwrap());
    }
    #[tokio::test]
    #[ignore = "manual controlled image queue latency measurement; no network"]
    async fn measure_visible_image_queue_latency() {
        let mut before = Vec::new();
        let mut after = Vec::new();
        for _ in 0..5 {
            let old = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            let mut held = Vec::new();
            for _ in 0..4 {
                held.push(old.clone().acquire_owned().await.unwrap());
            }
            let release = tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                drop(held);
            });
            let start = std::time::Instant::now();
            let permit = old.acquire().await.unwrap();
            before.push(start.elapsed().as_secs_f64() * 1000.0);
            drop(permit);
            release.await.unwrap();
            let a = image_network_permit(true).await.unwrap();
            let b = image_network_permit(true).await.unwrap();
            let start = std::time::Instant::now();
            let fg = image_network_permit(false).await.unwrap();
            after.push(start.elapsed().as_secs_f64() * 1000.0);
            drop((fg, a, b));
        }
        for (task, mut values) in [
            ("fifo_background_before", before),
            ("foreground_reserved_after", after),
        ] {
            values.sort_by(f64::total_cmp);
            println!(
                "{}",
                serde_json::json!({"task":task,"iterations":values.len(),"min_ms":values[0],"avg_ms":values.iter().sum::<f64>()/values.len() as f64,"p50_ms":values[2],"p95_ms":values[4],"p99_ms":values[4],"dataset":"4 background slots, simulated 50 ms service, no network","platform":std::env::consts::OS})
            );
        }
    }
}
