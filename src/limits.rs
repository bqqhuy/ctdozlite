//! Limits apply before allocating/decoding untrusted network data.
use std::io::{Cursor, Read};
pub static IMAGE_JOBS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

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
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    reader.decode()
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
