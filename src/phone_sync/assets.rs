//! Credential-free downloads for the pinned official codec and signed transfer files.
use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::OnceLock, time::Duration};

pub(super) const CODEC_SHA256: &str =
    "3a868037a32739a4fcb3c22425150745613006f9dbc561f30fd705effaed55fa";
const DOWNLOAD_LIMIT: usize = 64 * 1024 * 1024;

fn client() -> Result<&'static reqwest::Client> {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    if let Some(client) = CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()?;
    let _ = CLIENT.set(client);
    CLIENT.get().context("Public download client unavailable")
}
fn allowed(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|host| {
            crate::endpoints::SYNC_DOWNLOAD_DOMAINS
                .iter()
                .any(|suffix| {
                    host == *suffix
                        || host
                            .strip_suffix(suffix)
                            .is_some_and(|prefix| prefix.ends_with('.'))
                })
        })
}
/// Never attach the session cookie jar to a public asset or phone-signed URL.
pub(super) async fn download(url: &str, limit: usize) -> Result<Vec<u8>> {
    let mut url =
        reqwest::Url::parse(url).map_err(|_| anyhow::anyhow!("Invalid sync download URL"))?;
    for _ in 0..4 {
        ensure!(allowed(&url), "Sync download host rejected");
        // reqwest errors may include signed URLs; expose a fixed error instead.
        let response = client()?
            .get(url.clone())
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Sync download failed"))?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .context("Sync redirect missing location")?
                .to_str()
                .map_err(|_| anyhow::anyhow!("Invalid sync redirect"))?;
            url = url
                .join(location)
                .map_err(|_| anyhow::anyhow!("Invalid sync redirect"))?;
            continue;
        }
        ensure!(
            response.status().is_success(),
            "Sync download returned HTTP {}",
            response.status().as_u16()
        );
        return crate::limits::response_bytes(response, limit.min(DOWNLOAD_LIMIT))
            .await
            .map_err(|_| anyhow::anyhow!("Sync download incomplete or exceeds size limit"));
    }
    anyhow::bail!("Too many sync redirects")
}
pub(super) fn verify(bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() <= 1024 * 1024 && format!("{:x}", Sha256::digest(bytes)) == CODEC_SHA256,
        "Official sync codec hash mismatch"
    );
    Ok(())
}
pub(super) async fn codec(cache: &Path) -> Result<Vec<u8>> {
    let path = cache.join(format!("phone-codec-{CODEC_SHA256}.wasm"));
    let read = path.clone();
    if let Ok(Ok(bytes)) =
        tokio::task::spawn_blocking(move || crate::limits::read_file(&read, 1024 * 1024)).await
    {
        if verify(&bytes).is_ok() {
            return Ok(bytes);
        }
    }
    let bytes = download(crate::endpoints::PHONE_SYNC_CODEC, 1024 * 1024).await?;
    let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let bytes = if bytes.starts_with(&[0x1f, 0x8b]) {
            use std::io::Read;
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(bytes.as_slice())
                .take(1024 * 1024 + 1)
                .read_to_end(&mut out)?;
            out
        } else {
            bytes
        };
        verify(&bytes)?;
        crate::storage::atomic_write(&path, &bytes)?;
        Ok(bytes)
    })
    .await??;
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_credentials_ports_and_suffix_spoofing() {
        for url in [
            "http://chat.zalo.me/a",
            "https://zalo.me.evil.test/a",
            "https://notzalo.me/a",
            "https://u:p@chat.zalo.me/a",
            "https://chat.zalo.me:444/a",
            "https://127.0.0.1/a",
        ] {
            assert!(!allowed(&reqwest::Url::parse(url).unwrap()));
        }
        assert!(allowed(
            &reqwest::Url::parse("https://photo-stal-1.zdn.vn/a").unwrap()
        ));
        assert!(verify(b"arbitrary code").is_err());
    }
}
