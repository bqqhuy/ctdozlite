//! Credential-free, validated CDN redirects and bounded streaming downloads.
use anyhow::{ensure, Context, Result};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock,
    },
    time::Duration,
};
static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
pub static DOWNLOADS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
pub fn allowed(u: &reqwest::Url) -> bool {
    #[cfg(test)]
    if u.scheme() == "http" && u.host_str() == Some("127.0.0.1") {
        return true;
    }
    u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port_or_known_default() == Some(443)
        && u.host_str().is_some_and(|host| {
            crate::endpoints::MEDIA_DOWNLOAD_DOMAINS
                .iter()
                .any(|suffix| {
                    host == *suffix || host.strip_suffix(suffix).is_some_and(|p| p.ends_with('.'))
                })
        })
}
fn client() -> Result<&'static reqwest::Client> {
    if let Some(c) = CLIENT.get() {
        return Ok(c);
    }
    let builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(8))
        .user_agent("Mozilla/5.0")
        .referer(true);
    #[cfg(test)]
    let builder = builder.no_proxy();
    let c = crate::proxy::apply(builder)?.build()?;
    let _ = CLIENT.set(c);
    CLIENT.get().context("Không tạo được kết nối media")
}
pub async fn response(raw: &str, timeout: Duration) -> Result<reqwest::Response> {
    let mut url =
        reqwest::Url::parse(raw).map_err(|_| anyhow::anyhow!("Đường dẫn media không hợp lệ"))?;
    for _ in 0..5 {
        ensure!(allowed(&url), "Nguồn media không được hỗ trợ");
        let result = client()?
            .get(url.clone())
            .header(reqwest::header::REFERER, crate::endpoints::WEB_ORIGIN)
            .timeout(timeout)
            .send()
            .await;
        let r = result.map_err(|e| {
            if e.is_timeout() {
                anyhow::Error::from(crate::media_recovery::Failure::Timeout)
            } else {
                anyhow::Error::from(crate::media_recovery::Failure::Network)
            }
        })?;
        if r.status().is_redirection() {
            let location = r
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|h| h.to_str().ok())
                .context("Nguồn media chuyển hướng không hợp lệ")?;
            url = url
                .join(location)
                .map_err(|_| anyhow::anyhow!("Nguồn media chuyển hướng không hợp lệ"))?;
            continue;
        }
        if !r.status().is_success() {
            return Err(crate::media_recovery::Failure::Http(r.status().as_u16()).into());
        }
        return Ok(r);
    }
    anyhow::bail!("Nguồn media chuyển hướng quá nhiều lần")
}
struct Partial(PathBuf);
impl Drop for Partial {
    fn drop(&mut self) {
        // Best-effort cleanup owns only this task's unique partial.
        let _ = std::fs::remove_file(&self.0);
    }
}
pub fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(150)
        .collect();
    let s = s.trim().trim_matches('.');
    if s.is_empty() {
        "Tệp đính kèm".into()
    } else {
        s.into()
    }
}
/// Stream a general attachment without interpreting JSON documents as media errors.
/// # Errors
/// Returns transport, cancellation and filesystem failures without replacing a destination.
pub async fn save<F: FnMut(u64, Option<u64>)>(
    url: &str,
    path: &Path,
    cancel: Arc<AtomicBool>,
    progress: F,
) -> Result<u64> {
    save_classified(url, path, cancel, progress, false).await
}
/// Stream a visual attachment, rejecting JSON/HTML error bodies even with HTTP 200.
/// # Errors
/// Returns typed media failures or safe streaming/finalization failures.
pub async fn save_classified<F: FnMut(u64, Option<u64>)>(
    url: &str,
    path: &Path,
    cancel: Arc<AtomicBool>,
    progress: F,
    visual: bool,
) -> Result<u64> {
    save_inner(url, path, cancel, progress, visual, None, None).await
}
/// Stream a playback copy using its shared disk budget.
/// # Errors
/// Returns media failures or an exhausted playback cache budget.
pub(crate) async fn save_video<F: FnMut(u64, Option<u64>)>(
    url: &str,
    path: &Path,
    cancel: Arc<AtomicBool>,
    progress: F,
    lease: &crate::video_player::CachedVideo,
) -> Result<u64> {
    save_inner(url, path, cancel, progress, true, Some(lease), None).await
}
/// Stream an attachment copy with a 512 MiB per-file cache limit.
pub(crate) async fn save_cached<F: FnMut(u64, Option<u64>)>(
    url: &str,
    path: &Path,
    cancel: Arc<AtomicBool>,
    progress: F,
    visual: bool,
) -> Result<u64> {
    save_inner(
        url,
        path,
        cancel,
        progress,
        visual,
        None,
        Some(512 * 1024 * 1024),
    )
    .await
}
async fn save_inner<F: FnMut(u64, Option<u64>)>(
    url: &str,
    path: &Path,
    cancel: Arc<AtomicBool>,
    mut progress: F,
    visual: bool,
    lease: Option<&crate::video_player::CachedVideo>,
    byte_limit: Option<u64>,
) -> Result<u64> {
    use tokio::io::AsyncWriteExt;
    let _slot = DOWNLOADS.acquire().await?;
    ensure!(!cancel.load(Ordering::Acquire), "Đã hủy tải");
    let mut response = response(url, Duration::from_secs(3600)).await?;
    let expected = response.content_length();
    ensure!(
        !byte_limit
            .zip(expected)
            .is_some_and(|(limit, size)| size > limit),
        "Tệp vượt giới hạn cache 512 MiB; hãy dùng Lưu thành…"
    );
    let mut random = [0; 12];
    crate::random::fill(&mut random)?;
    let suffix = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let partial = Partial(path.with_file_name(format!(".ctdoz-{suffix}.partial")));
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&partial.0)
        .await
        .context("Không tạo được tệp tải về")?;
    let mut total = 0u64;
    let mut last = std::time::Instant::now() - Duration::from_secs(1);
    loop {
        ensure!(!cancel.load(Ordering::Acquire), "Đã hủy tải");
        let cancelled = async {
            loop {
                if cancel.load(Ordering::Acquire) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        let chunk = tokio::select! {r=response.chunk()=>r.map_err(|_|anyhow::anyhow!("Tải tệp bị gián đoạn"))?,_=cancelled=>anyhow::bail!("Đã hủy tải"),_=tokio::time::sleep(Duration::from_secs(120))=>anyhow::bail!("Tải tệp hết thời gian chờ")};
        let Some(chunk) = chunk else {
            break;
        };
        if visual && total == 0 {
            let first = chunk.iter().find(|b| !b.is_ascii_whitespace());
            if matches!(first, Some(b'{' | b'[' | b'<')) {
                let mut body = chunk.to_vec();
                while body.len() <= 65536 {
                    let next = tokio::time::timeout(Duration::from_secs(20), response.chunk())
                        .await
                        .map_err(|_| crate::media_recovery::Failure::Timeout)?
                        .map_err(|_| crate::media_recovery::Failure::Network)?;
                    let Some(next) = next else {
                        break;
                    };
                    if next.len() > 65536usize.saturating_sub(body.len()) {
                        return Err(crate::media_recovery::Failure::Content.into());
                    }
                    body.extend_from_slice(&next);
                }
                crate::media_recovery::validate_image_body(&body)?;
                return Err(crate::media_recovery::Failure::Content.into());
            }
        }
        ensure!(
            !byte_limit.is_some_and(|limit| total.saturating_add(chunk.len() as u64) > limit),
            "Tệp vượt giới hạn cache 512 MiB; hãy dùng Lưu thành…"
        );
        if let Some(lease) = lease {
            lease.admit(chunk.len())?;
        }
        file.write_all(&chunk)
            .await
            .context("Không ghi được tệp; kiểm tra dung lượng đĩa")?;
        total = total
            .checked_add(chunk.len() as u64)
            .context("Dung lượng tệp không hợp lệ")?;
        if last.elapsed() >= Duration::from_millis(200) {
            progress(total, expected);
            last = std::time::Instant::now();
        }
    }
    ensure!(
        expected.is_none_or(|n| n == total),
        "Tệp tải về chưa đủ dữ liệu"
    );
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    ensure!(!cancel.load(Ordering::Acquire), "Đã hủy tải");
    // Persist without replacing an existing user file, including races after the picker.
    #[cfg(not(windows))]
    tokio::fs::hard_link(&partial.0, path)
        .await
        .context("Tệp đích đã tồn tại hoặc không thể hoàn tất; chọn tên khác")?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        extern "system" {
            fn MoveFileW(from: *const u16, to: *const u16) -> i32;
        }
        let from: Vec<u16> = partial.0.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        ensure!(
            !from[..from.len() - 1].contains(&0) && !to[..to.len() - 1].contains(&0),
            "Tên tệp không hợp lệ"
        );
        // SAFETY: both paths remain NUL-terminated and alive during this non-replacing move.
        ensure!(
            unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } != 0,
            "Tệp đích đã tồn tại hoặc không thể hoàn tất; chọn tên khác"
        );
    }
    progress(total, expected);
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_credentialed_or_lookalike_destinations() {
        for s in [
            "http://photo.zdn.vn/a",
            "https://zdn.vn.evil.invalid/a",
            "https://user@photo.zdn.vn/a",
        ] {
            assert!(!allowed(&reqwest::Url::parse(s).unwrap()));
        }
        assert!(allowed(
            &reqwest::Url::parse("https://photo.zdn.vn/a").unwrap()
        ));
    }
    #[test]
    fn filenames_cannot_escape_picker_directory() {
        assert_eq!(safe_name("../a\\b:?.png"), "ab.png");
    }
    #[tokio::test]
    async fn streaming_save_preserves_existing_file_and_cleans_partial() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for attempt in 0..3 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let _ = socket.read(&mut request);
                let body: &[u8] = if attempt == 2 { b"abc" } else { b"abcdef" };
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\n")
                    .unwrap();
                socket.write_all(body).unwrap();
            }
        });
        let root = std::env::temp_dir().join(format!(
            "ctdoz-save-{}-{}",
            std::process::id(),
            address.port()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("file.bin");
        let url = format!("http://{address}/file");
        let cancel = Arc::new(AtomicBool::new(false));
        assert_eq!(
            save(&url, &path, cancel.clone(), |_, _| {}).await.unwrap(),
            6
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"abcdef");
        assert!(save(&url, &path, cancel, |_, _| {}).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"abcdef");
        let truncated = root.join("truncated.bin");
        assert!(save(
            &url,
            &truncated,
            Arc::new(AtomicBool::new(false)),
            |_, _| {}
        )
        .await
        .is_err());
        assert!(!truncated.exists());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        server.join().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}


/// Stream a specific byte range of a media resource (RFC 7233).
#[allow(dead_code)]
pub async fn range_request(
    raw: &str,
    start_byte: u64,
    end_byte: Option<u64>,
    timeout: Duration,
) -> Result<reqwest::Response> {
    let url = reqwest::Url::parse(raw).map_err(|_| anyhow::anyhow!("Đường dẫn media không hợp lệ"))?;
    ensure!(allowed(&url), "Nguồn media không được hỗ trợ");
    let range_val = match end_byte {
        Some(end) => format!("bytes={start_byte}-{end}"),
        None => format!("bytes={start_byte}-"),
    };
    let r = client()?
        .get(url)
        .header(reqwest::header::REFERER, crate::endpoints::WEB_ORIGIN)
        .header(reqwest::header::RANGE, range_val)
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Lỗi tải đoạn media: {e}"))?;
    ensure!(
        r.status().is_success() || r.status() == reqwest::StatusCode::PARTIAL_CONTENT,
        "Máy chủ không hỗ trợ Range request"
    );
    Ok(r)
}

#[cfg(test)]
mod response_body_tests {
    use super::*;
    #[tokio::test]
    async fn http_200_json_is_a_remote_error_for_video_but_a_valid_general_attachment() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().unwrap();
                let mut request = [0; 1024];
                let _ = socket.read(&mut request);
                let body = br#"{"err_code":17,"message":"unavailable"}"#;
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                socket.write_all(body).unwrap();
            }
        });
        let root = std::env::temp_dir().join(format!(
            "media-json-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("video.mp4");
        let cancel = Arc::new(AtomicBool::new(false));
        let url = format!("http://{address}/sample");
        let error = save_classified(&url, &path, cancel.clone(), |_, _| {}, true)
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<crate::media_recovery::Failure>(),
            Some(&crate::media_recovery::Failure::Json(17))
        );
        assert!(!path.exists());
        let json = root.join("data.json");
        assert!(save(&url, &json, cancel, |_, _| {}).await.is_ok());
        server.join().unwrap();
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn range_request_sends_proper_headers_and_handles_206() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request);
            let req_str = String::from_utf8_lossy(&request);
            assert!(req_str.contains("range: bytes=100-200"));
            let body = b"partial content chunk";
            write!(
                socket,
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 100-200/500\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            socket.write_all(body).unwrap();
        });
        let url = format!("http://{address}/video.mp4");
        let resp = range_request(&url, 100, Some(200), Duration::from_secs(5)).await.unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        server.join().unwrap();
    }
}
