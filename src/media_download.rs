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

/// DoH-pinned clients, one per hostname, reused across downloads.
///
/// The pinned addresses go stale when the CDN re-points a hostname, so each
/// entry carries an expiry matching the DoH answer's TTL.
static DOH_CLIENTS: OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, reqwest::Client)>>,
> = OnceLock::new();

fn doh_clients() -> &'static std::sync::Mutex<
    std::collections::HashMap<String, (std::time::Instant, reqwest::Client)>,
> {
    DOH_CLIENTS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

/// Build (or reuse) a client that pins `host` to the DoH-resolved addresses.
///
/// `ttl` bounds how long the pinned addresses may be reused.
fn doh_client_for(
    host: &str,
    addrs: &[std::net::IpAddr],
    ttl: std::time::Duration,
) -> Result<reqwest::Client> {
    let now = std::time::Instant::now();
    {
        let cache = crate::doh_resolver::lock_cache(doh_clients());
        if let Some((expires, client)) = cache.get(host) {
            if *expires > now {
                return Ok(client.clone());
            }
        }
    }
    let addr_port: Vec<std::net::SocketAddr> = addrs
        .iter()
        .map(|ip| std::net::SocketAddr::new(*ip, 443))
        .collect();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(8))
        .user_agent("Mozilla/5.0")
        .referer(true)
        .resolve_to_addrs(host, &addr_port)
        .no_proxy()
        .build()
        .context("Không tạo được DoH client")?;
    crate::doh_resolver::lock_cache(doh_clients())
        .insert(host.to_owned(), (now + ttl, client.clone()));
    Ok(client)
}
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
        let r = match result {
            Ok(r) => r,
            Err(ref e) if is_dns_error(e) => {
                // System DNS failed for a known CDN domain — attempt DoH fallback.
                if let Some(host) = url.host_str() {
                    let host_str = host.to_owned();
                    match crate::doh_resolver::shared()?
                        .resolve_with_ttl(&host_str)
                        .await
                    {
                        Ok((addrs, ttl)) => {
                            // Reuse a pinned client for this hostname; the
                            // resolver itself caches the addresses.
                            let doh_client = doh_client_for(
                                &host_str,
                                &addrs,
                                Duration::from_secs(ttl.clamp(60, 600) as u64),
                            )?;
                            doh_client
                                .get(url.clone())
                                .header(reqwest::header::REFERER, crate::endpoints::WEB_ORIGIN)
                                .timeout(timeout)
                                .send()
                                .await
                                .map_err(|e| {
                                    if e.is_timeout() {
                                        anyhow::Error::from(crate::media_recovery::Failure::Timeout)
                                    } else {
                                        // Invalidate DoH cache for this host on connection failure.
                                        if let Ok(resolver) = crate::doh_resolver::shared() {
                                            resolver.invalidate(&host_str);
                                        }
                                        // The pinned client still holds the stale
                                        // addresses — drop it so the next attempt
                                        // re-resolves.
                                        crate::doh_resolver::lock_cache(doh_clients())
                                            .remove(&host_str);
                                        anyhow::Error::from(crate::media_recovery::Failure::Network)
                                    }
                                })?
                        }
                        Err(_) => {
                            return Err(crate::media_recovery::Failure::Network.into());
                        }
                    }
                } else {
                    return Err(crate::media_recovery::Failure::Network.into());
                }
            }
            Err(e) => {
                return Err(if e.is_timeout() {
                    crate::media_recovery::Failure::Timeout.into()
                } else {
                    crate::media_recovery::Failure::Network.into()
                });
            }
        };
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

const MAX_PREVIEW_BYTES: usize = 1024 * 1024;

/// Read a bounded document prefix, falling back to validated CDN transport if cache reading fails.
/// # Errors
/// Returns safe transport/read errors; partial failed transfers are never successful previews.
pub(crate) async fn document_preview(cached: &Path, url: &str) -> Result<Vec<u8>> {
    use tokio::io::AsyncReadExt;
    async fn cached_prefix(file: tokio::fs::File) -> std::io::Result<Vec<u8>> {
        if !file.metadata().await?.is_file() {
            return Err(std::io::Error::other("Not a regular cached file"));
        }
        let mut bytes = Vec::new();
        file.take(MAX_PREVIEW_BYTES as u64)
            .read_to_end(&mut bytes)
            .await?;
        Ok(bytes)
    }
    // Cache eviction can race this read. An unreadable optional copy must not block refetching.
    if let Ok(file) = tokio::fs::File::open(cached).await {
        if let Ok(bytes) = cached_prefix(file).await {
            if !bytes.is_empty() {
                return Ok(bytes);
            }
        }
    }
    let read = async {
        let mut response = response(url, Duration::from_secs(10)).await?;
        let mut bytes = Vec::new();
        while bytes.len() < MAX_PREVIEW_BYTES {
            let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| anyhow::anyhow!("Không đọc được nội dung xem trước tài liệu"))?
            else {
                break;
            };
            let remaining = MAX_PREVIEW_BYTES - bytes.len();
            bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        }
        Ok(bytes)
    };
    tokio::time::timeout(Duration::from_secs(10), read)
        .await
        .map_err(|_| anyhow::anyhow!("Quá thời gian tải xem trước tài liệu"))?
}

/// Returns true when the reqwest error indicates a DNS resolution failure.
///
/// reqwest exposes no "is DNS" flag, so the error chain is inspected for the
/// resolver-level markers.  Treating *every* connect error as a DNS failure
/// would fire the DoH fallback for TLS and refused-connection errors, which
/// DoH cannot fix.
fn is_dns_error(e: &reqwest::Error) -> bool {
    if !e.is_connect() || e.is_timeout() {
        return false;
    }
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(err) = source {
        let text = err.to_string().to_ascii_lowercase();
        if text.contains("dns error")
            || text.contains("failed to lookup address")
            || text.contains("name or service not known")
            || text.contains("nodename nor servname")
            || text.contains("temporary failure in name resolution")
            || text.contains("no such host")
        {
            return true;
        }
        source = err.source();
    }
    false
}
static ACTIVE_DOWNLOADS: OnceLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
    OnceLock::new();

fn active_downloads() -> &'static std::sync::Mutex<std::collections::HashSet<PathBuf>> {
    ACTIVE_DOWNLOADS.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

struct DownloadLock(PathBuf);
impl Drop for DownloadLock {
    fn drop(&mut self) {
        if let Ok(mut set) = active_downloads().lock() {
            set.remove(&self.0);
        }
    }
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
    let canonical_dest = path.to_path_buf();
    let _download_lock = {
        let mut set = active_downloads()
            .lock()
            .map_err(|_| anyhow::anyhow!("Lỗi khóa tải tệp"))?;
        ensure!(
            set.insert(canonical_dest.clone()),
            "Tệp đang được tải về bởi một tác vụ khác"
        );
        DownloadLock(canonical_dest)
    };
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
    if let Some(lease) = lease {
        if let Some(expected_len) = expected {
            lease.reserve_capacity(expected_len)?;
        }
    }
    let file_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("media");
    let safe_id: String = file_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(32)
        .collect();
    let safe_id = if safe_id.is_empty() {
        "media".into()
    } else {
        safe_id
    };
    let pid = std::process::id();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut random = [0; 8];
    crate::random::fill(&mut random)?;
    let suffix = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let partial = Partial(path.with_file_name(format!(
        ".ctdoz-{safe_id}-{pid}-{timestamp}-{suffix}.partial"
    )));
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
    async fn preview_server(reply: Vec<u8>) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut request = [0; 4096];
            let received = stream.read(&mut request).await.unwrap();
            assert!(
                received > 0,
                "preview must send a request before the fixture replies"
            );
            // A prefix reader may close after its byte limit.
            let _ = stream.write_all(&reply).await;
        });
        (format!("http://{address}/document.txt"), server)
    }

    #[tokio::test]
    #[ignore = "controlled local preview cache benchmark"]
    async fn measure_document_preview_cache() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("large.txt");
        tokio::fs::write(&path, vec![b'x'; 8 * MAX_PREVIEW_BYTES])
            .await
            .unwrap();
        for bounded in [false, true] {
            let mut samples = Vec::new();
            for _ in 0..7 {
                let start = std::time::Instant::now();
                let bytes = if bounded {
                    document_preview(&path, "invalid-url").await.unwrap()
                } else {
                    tokio::fs::read(&path).await.unwrap()
                };
                std::hint::black_box(bytes);
                samples.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            samples.sort_by(f64::total_cmp);
            println!(
                "{}",
                serde_json::json!({
                    "task": if bounded { "preview_bounded_after" } else { "preview_unbounded_before" },
                    "iterations": 7, "dataset_bytes": 8 * MAX_PREVIEW_BYTES,
                    "retained_bytes": if bounded { MAX_PREVIEW_BYTES } else { 8 * MAX_PREVIEW_BYTES },
                    "arch": std::env::consts::ARCH, "os": std::env::consts::OS, "profile": "debug",
                    "min_ms": samples[0], "avg_ms": samples.iter().sum::<f64>() / 7.0,
                    "p50_ms": samples[3], "p95_ms": samples[6], "p99_ms": samples[6]
                })
            );
        }
    }

    #[tokio::test]
    async fn document_preview_uses_bounded_cache_without_network() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("large.txt");
        tokio::fs::write(&path, vec![b'x'; MAX_PREVIEW_BYTES + 32])
            .await
            .unwrap();
        let bytes = document_preview(&path, "invalid-url").await.unwrap();
        assert_eq!(bytes, vec![b'x'; MAX_PREVIEW_BYTES]);
    }

    #[tokio::test]
    async fn document_preview_refetches_missing_unreadable_and_empty_cache() {
        let root = tempfile::tempdir().unwrap();
        let empty = root.path().join("empty.txt");
        tokio::fs::write(&empty, []).await.unwrap();
        for path in [
            root.path().join("evicted.txt"),
            root.path().to_owned(),
            empty,
        ] {
            let (url, server) = preview_server(
                b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello".to_vec(),
            )
            .await;
            assert_eq!(document_preview(&path, &url).await.unwrap(), b"hello");
            server.await.unwrap();
        }
    }

    #[tokio::test]
    async fn document_preview_rejects_http_errors_and_incomplete_streams() {
        let root = tempfile::tempdir().unwrap();
        for reply in [
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad".to_vec(),
            b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\nshort".to_vec(),
        ] {
            let (url, server) = preview_server(reply).await;
            assert!(document_preview(&root.path().join("missing"), &url)
                .await
                .is_err());
            server.await.unwrap();
        }
        assert!(
            document_preview(&root.path().join("missing"), "file:///private.txt")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn document_preview_bounds_download_prefix() {
        let root = tempfile::tempdir().unwrap();
        let mut reply = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_PREVIEW_BYTES + 32
        )
        .into_bytes();
        reply.extend(vec![b'x'; MAX_PREVIEW_BYTES + 32]);
        let (url, server) = preview_server(reply).await;
        assert_eq!(
            document_preview(&root.path().join("missing"), &url)
                .await
                .unwrap()
                .len(),
            MAX_PREVIEW_BYTES
        );
        server.await.unwrap();
    }

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

    /// The DoH fallback must fire for resolver failures only.  A TLS or
    /// refused-connection error is not fixable by resolving the name.
    #[tokio::test]
    async fn dns_failures_are_told_apart_from_other_connect_errors() {
        // A connect error to a host that does not resolve carries a DNS marker.
        let err = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
            .get("https://this-host-does-not-exist-zalolite.invalid/")
            .send()
            .await
            .unwrap_err();
        assert!(err.is_connect());
        assert!(
            is_dns_error(&err),
            "expected a DNS-classified error, got: {err}"
        );
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

/// Validate Content-Range header format: `bytes START-END/TOTAL` (RFC 7233).
///
/// Ensures START <= END, END < TOTAL, and START/END match the requested range.
pub fn validate_content_range(
    header_val: &str,
    req_start: u64,
    req_end: Option<u64>,
) -> Result<(u64, u64, u64)> {
    let s = header_val.trim();
    let stripped = s
        .strip_prefix("bytes ")
        .context("Content-Range header không đúng định dạng: thiếu tiền tố 'bytes '")?;
    let (range_str, total_str) = stripped
        .split_once('/')
        .context("Content-Range header không đúng định dạng: thiếu '/'")?;
    let (start_str, end_str) = range_str
        .split_once('-')
        .context("Content-Range header không đúng định dạng: thiếu '-'")?;

    let start: u64 = start_str
        .trim()
        .parse()
        .context("Content-Range START không phải là số hợp lệ")?;
    let end: u64 = end_str
        .trim()
        .parse()
        .context("Content-Range END không phải là số hợp lệ")?;
    let total: u64 = total_str
        .trim()
        .parse()
        .context("Content-Range TOTAL không phải là số hợp lệ")?;

    ensure!(
        start <= end,
        "Content-Range không hợp lệ: START ({start}) > END ({end})"
    );
    ensure!(
        end < total,
        "Content-Range không hợp lệ: END ({end}) >= TOTAL ({total})"
    );
    ensure!(
        start == req_start,
        "Content-Range START ({start}) không khớp với range yêu cầu ({req_start})"
    );
    if let Some(expected_end) = req_end {
        ensure!(
            end == expected_end,
            "Content-Range END ({end}) không khớp với range yêu cầu ({expected_end})"
        );
    }

    Ok((start, end, total))
}

/// Stream a specific byte range of a media resource (RFC 7233).
/// RFC 7233 byte-range request for seeking into remote media without full download.
///
/// WP15.1-B: Implementation is correct and tested, but no video player calls it yet.
/// Wiring requires: custom IMFByteStream for MFT backed by HTTP range requests, or
/// a different player architecture that can seek into remote URLs. Until wired, this
/// is prep code — the v0.015 CHANGELOG claim "Tua video tức thì" is not yet true.
/// TODO(wiring): integrate into video_player when player supports streaming sources.
#[allow(dead_code)]
pub async fn range_request(
    raw: &str,
    start_byte: u64,
    end_byte: Option<u64>,
    timeout: Duration,
) -> Result<reqwest::Response> {
    let url =
        reqwest::Url::parse(raw).map_err(|_| anyhow::anyhow!("Đường dẫn media không hợp lệ"))?;
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
    if r.status() == reqwest::StatusCode::PARTIAL_CONTENT {
        let content_range = r
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|h| h.to_str().ok())
            .context("Thiếu header Content-Range trong phản hồi 206 Partial Content")?;
        validate_content_range(content_range, start_byte, end_byte)?;
    }
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
        let resp = range_request(&url, 100, Some(200), Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(resp.status(), reqwest::StatusCode::PARTIAL_CONTENT);
        server.join().unwrap();
    }

    #[test]
    fn validate_content_range_accepts_valid_and_rejects_malformed() {
        // Valid closed range
        let res = validate_content_range("bytes 100-200/500", 100, Some(200)).unwrap();
        assert_eq!(res, (100, 200, 500));

        // Valid open-ended request range
        let res = validate_content_range("bytes 0-499/1000", 0, None).unwrap();
        assert_eq!(res, (0, 499, 1000));

        // Missing "bytes " prefix
        assert!(validate_content_range("100-200/500", 100, Some(200)).is_err());

        // Missing total
        assert!(validate_content_range("bytes 100-200", 100, Some(200)).is_err());

        // START > END
        assert!(validate_content_range("bytes 200-100/500", 200, Some(100)).is_err());

        // END >= TOTAL
        assert!(validate_content_range("bytes 100-500/500", 100, Some(500)).is_err());
        assert!(validate_content_range("bytes 100-600/500", 100, Some(600)).is_err());

        // START mismatch
        assert!(validate_content_range("bytes 101-200/500", 100, Some(200)).is_err());

        // END mismatch
        assert!(validate_content_range("bytes 100-201/500", 100, Some(200)).is_err());

        // Non-numeric fields
        assert!(validate_content_range("bytes abc-200/500", 100, Some(200)).is_err());
        assert!(validate_content_range("bytes 100-def/500", 100, Some(200)).is_err());
        assert!(validate_content_range("bytes 100-200/ghi", 100, Some(200)).is_err());
    }

    #[tokio::test]
    async fn range_request_rejects_missing_or_mismatched_content_range() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            // Attempt 1: 206 missing Content-Range
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request);
            let body = b"chunk without header";
            write!(
                socket,
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            socket.write_all(body).unwrap();

            // Attempt 2: 206 with mismatched Content-Range (claims 300-400 instead of 100-200)
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request);
            let body = b"mismatched chunk";
            write!(
                socket,
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes 300-400/500\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            socket.write_all(body).unwrap();
        });

        let url = format!("http://{address}/video.mp4");
        // Missing Content-Range -> Err
        assert!(range_request(&url, 100, Some(200), Duration::from_secs(5))
            .await
            .is_err());
        // Mismatched Content-Range -> Err
        assert!(range_request(&url, 100, Some(200), Duration::from_secs(5))
            .await
            .is_err());
        server.join().unwrap();
    }

    #[tokio::test]
    async fn concurrent_downloads_to_same_path_are_locked() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request);
            let body = b"slow download data";
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            // Send first half, delay to simulate in-flight download
            socket.write_all(&body[..4]).unwrap();
            std::thread::sleep(Duration::from_millis(200));
            socket.write_all(&body[4..]).unwrap();
        });

        let root = std::env::temp_dir().join(format!(
            "ctdoz-lock-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("file.bin");
        let url = format!("http://{address}/file");
        let cancel = Arc::new(AtomicBool::new(false));

        let path_clone = path.clone();
        let url_clone = url.clone();
        let cancel_clone = cancel.clone();
        let handle1 =
            tokio::spawn(
                async move { save(&url_clone, &path_clone, cancel_clone, |_, _| {}).await },
            );

        // Give handle1 a moment to acquire the lock and start downloading
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Second download to the exact same path must be rejected by DownloadLock
        let result2 = save(&url, &path, cancel.clone(), |_, _| {}).await;
        assert!(result2.is_err());
        let err_msg = result2.unwrap_err().to_string();
        assert!(
            err_msg.contains("đang được tải về bởi một tác vụ khác"),
            "Unexpected error: {err_msg}"
        );

        let result1 = handle1.await.unwrap();
        assert!(result1.is_ok());
        server.join().unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
