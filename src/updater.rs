//! Bounded release downloads and transactional installation, independent of GUI and processes.

use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Resource limits for one release asset.
#[derive(Clone, Copy)]
pub struct DownloadLimits {
    /// Smallest accepted artifact.
    pub min_bytes: u64,
    /// Maximum buffered artifact size.
    pub max_bytes: u64,
    /// Deadline covering headers and the complete body.
    pub timeout: Duration,
}

impl Default for DownloadLimits {
    fn default() -> Self {
        Self {
            min_bytes: 512 * 1024,
            max_bytes: 512 * 1024 * 1024,
            timeout: Duration::from_secs(240),
        }
    }
}

/// Release metadata supplied by the release discovery layer.
pub struct DownloadRequest<'a> {
    /// HTTP asset location, without session cookies.
    pub url: &'a str,
    /// A single filename, never a path.
    pub asset_name: &'a str,
    /// Optional published SHA-256 hex digest.
    pub expected_sha256: Option<&'a str>,
    /// Optional size published by the release API.
    pub expected_size: Option<u64>,
    /// Resource bounds for this transfer.
    pub limits: DownloadLimits,
}

/// Explicit destination selected by the platform adapter, not remote metadata.
pub enum InstallTarget {
    /// Replace a portable executable and preserve its permissions.
    Portable(PathBuf),
    /// Save an installer for the platform launcher.
    Installer(PathBuf),
}

/// Reject absolute paths, traversal and cross-platform path separators.
///
/// # Errors
/// Returns an error unless the input is a single ordinary filename.
pub fn validate_asset_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', ':', '\0'])
        || name.chars().any(char::is_control)
    {
        return Err("Tên gói cập nhật không hợp lệ".into());
    }
    Ok(())
}

/// Download and validate an asset before any installation destination is touched.
/// The caller supplies a dedicated, cookie-free client and a progress callback.
///
/// # Errors
/// Rejects malformed metadata, HTTP failures, incomplete bodies, resource limits,
/// unsupported executable signatures, and checksum mismatches.
pub async fn download_release(
    client: &reqwest::Client,
    request: &DownloadRequest<'_>,
    mut progress: impl FnMut(u64, u64),
) -> Result<Vec<u8>, String> {
    validate_asset_name(request.asset_name)?;
    let limits = request.limits;
    if limits.min_bytes > limits.max_bytes || limits.max_bytes > usize::MAX as u64 {
        return Err("Giới hạn dung lượng cập nhật không hợp lệ".into());
    }
    if let Some(hash) = request.expected_sha256 {
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("Mã SHA-256 của gói cập nhật không hợp lệ".into());
        }
    }
    if request
        .expected_size
        .is_some_and(|size| size > limits.max_bytes)
    {
        return Err("Gói cập nhật vượt giới hạn dung lượng".into());
    }
    let transfer = async {
        let mut response = client
            .get(request.url)
            .timeout(limits.timeout)
            .send()
            .await
            .map_err(|e| format!("Tải gói cập nhật thất bại: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Máy chủ cập nhật trả về lỗi: {e}"))?;
        let length = response.content_length();
        if length.is_some_and(|size| size > limits.max_bytes) {
            return Err("Gói cập nhật vượt giới hạn dung lượng".into());
        }
        if let (Some(expected), Some(actual)) = (request.expected_size, length) {
            if expected != actual {
                return Err("Dung lượng gói cập nhật không khớp metadata".into());
            }
        }
        let total = request.expected_size.or(length).unwrap_or(0);
        let mut bytes = Vec::new();
        let mut hasher = Sha256::new();
        let mut last_progress = std::time::Instant::now();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|e| format!("Ngắt kết nối khi đang tải cập nhật: {e}"))?
        {
            if (chunk.len() as u64) > limits.max_bytes.saturating_sub(bytes.len() as u64) {
                return Err("Gói cập nhật vượt giới hạn dung lượng".into());
            }
            hasher.update(&chunk);
            bytes.extend_from_slice(&chunk);
            if last_progress.elapsed() >= Duration::from_millis(80) {
                progress(bytes.len() as u64, total);
                last_progress = std::time::Instant::now();
            }
        }
        let actual = bytes.len() as u64;
        if actual < limits.min_bytes {
            return Err("Gói cập nhật tải về quá nhỏ".into());
        }
        if length.is_some_and(|size| size != actual)
            || request.expected_size.is_some_and(|size| size != actual)
        {
            return Err("Gói cập nhật tải về chưa hoàn tất".into());
        }
        if let Some(expected) = request.expected_sha256 {
            let actual = format!("{:x}", hasher.finalize());
            if !actual.eq_ignore_ascii_case(expected) {
                return Err("Mã kiểm tra toàn vẹn SHA-256 không khớp".into());
            }
        }
        let name = request.asset_name.to_ascii_lowercase();
        if name.ends_with(".exe") && !bytes.starts_with(b"MZ") {
            return Err("File cập nhật không có chữ ký PE hợp lệ".into());
        }
        if name.ends_with(".appimage") && !bytes.starts_with(b"\x7fELF") {
            return Err("File cập nhật không có chữ ký ELF hợp lệ".into());
        }
        progress(actual, total.max(actual));
        Ok(bytes)
    };
    tokio::time::timeout(limits.timeout, transfer)
        .await
        .map_err(|_| "Tải gói cập nhật quá thời gian cho phép".to_string())?
}

static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

fn staging_file(destination: &Path) -> Result<(PathBuf, File), String> {
    let parent = destination
        .parent()
        .ok_or("Đường dẫn cập nhật thiếu thư mục")?;
    for _ in 0..100 {
        let id = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".ctdo-update-{}-{id}", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Tạo file cập nhật tạm thất bại: {error}")),
        }
    }
    Err("Không tạo được file cập nhật tạm riêng biệt".into())
}

fn remove_staging(path: &Path) {
    // Cleanup must not hide the primary download/install failure.
    let _ = fs::remove_file(path);
}

/// Portable installation staged without replacing or launching the running executable.
/// Dropping an uncommitted transaction preserves the original executable.
pub struct DeferredPortableInstall {
    prepared: PreparedInstall,
}

impl DeferredPortableInstall {
    /// Replace the executable and launch only after the caller has joined its worker.
    ///
    /// # Errors
    /// Returns replacement, launch or explicit rollback errors.
    pub fn launch_and_finish(
        self,
        launch: impl FnOnce(&Path) -> Result<(), String>,
    ) -> Result<(), String> {
        self.prepared.finish(launch)
    }

    /// Discard the staged update and preserve the original executable.
    ///
    /// # Errors
    /// Returns a rollback error if replacement happened before cancellation.
    pub fn abort(mut self) -> Result<(), String> {
        self.prepared.rollback()
    }
}

/// Stage validated portable bytes without replacing or launching any executable.
///
/// # Errors
/// Returns filesystem errors while preparing private sibling files.
pub fn stage_portable_release(
    bytes: &[u8],
    destination: &Path,
) -> Result<DeferredPortableInstall, String> {
    Ok(DeferredPortableInstall {
        prepared: prepare_install(bytes, destination, true)?,
    })
}

struct PreparedInstall {
    destination: PathBuf,
    stage: PathBuf,
    backup: Option<PathBuf>,
    installed: bool,
    committed: bool,
}

impl PreparedInstall {
    fn finish(mut self, launch: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
        fs::rename(&self.stage, &self.destination)
            .map_err(|error| format!("Thay thế gói cập nhật thất bại: {error}"))?;
        self.installed = true;
        if let Err(error) = launch(&self.destination) {
            self.rollback().map_err(|rollback| {
                format!("Khởi chạy thất bại: {error}; hoàn tác thất bại: {rollback}")
            })?;
            return Err(format!("Khởi chạy thất bại, đã hoàn tác cập nhật: {error}"));
        }
        self.committed = true;
        Ok(())
    }

    fn rollback(&mut self) -> Result<(), String> {
        if self.installed && !self.committed {
            match &self.backup {
                Some(backup) => fs::rename(backup, &self.destination),
                None => fs::remove_file(&self.destination),
            }
            .map_err(|error| format!("Khôi phục gói cập nhật cũ thất bại: {error}"))?;
            self.installed = false;
        }
        Ok(())
    }
}

impl Drop for PreparedInstall {
    fn drop(&mut self) {
        // A retained backup is the recovery source if rollback itself fails.
        if self.rollback().is_err() {
            return;
        }
        remove_staging(&self.stage);
        if let Some(backup) = &self.backup {
            remove_staging(backup);
        }
    }
}

fn prepare_install(
    bytes: &[u8],
    destination: &Path,
    portable: bool,
) -> Result<PreparedInstall, String> {
    let metadata = match fs::symlink_metadata(destination) {
        Ok(metadata) if metadata.file_type().is_file() => Some(metadata),
        Ok(_) => return Err("Đích cập nhật không phải file thông thường".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !portable => None,
        Err(error) => return Err(format!("Đọc đích cập nhật thất bại: {error}")),
    };
    let (stage, mut file) = staging_file(destination)?;
    let prepared = (|| {
        file.write_all(bytes)?;
        if portable {
            if let Some(metadata) = &metadata {
                file.set_permissions(metadata.permissions())?;
            }
        }
        file.sync_all()
    })();
    drop(file);
    if let Err(error) = prepared {
        remove_staging(&stage);
        return Err(format!("Ghi file cập nhật thất bại: {error}"));
    }
    let backup = if metadata.is_some() {
        let (path, file) = match staging_file(destination) {
            Ok(value) => value,
            Err(error) => {
                remove_staging(&stage);
                return Err(error);
            }
        };
        drop(file);
        // A private sibling copy keeps the original destination present until atomic replacement.
        if let Err(error) = fs::copy(destination, &path) {
            remove_staging(&stage);
            remove_staging(&path);
            return Err(format!("Sao lưu bản cập nhật cũ thất bại: {error}"));
        }
        Some(path)
    } else {
        None
    };
    Ok(PreparedInstall {
        destination: destination.to_path_buf(),
        stage,
        backup,
        installed: false,
        committed: false,
    })
}

/// Atomically install already-validated bytes and invoke the injected launcher.
/// Existing bytes are restored if replacement or launch fails. A rollback failure
/// is returned explicitly and the backup is retained for recovery.
///
/// # Errors
/// Returns filesystem or launcher errors; never falls back to another destination.
pub fn install_release(
    bytes: &[u8],
    target: &InstallTarget,
    launch: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let (destination, portable) = match target {
        InstallTarget::Portable(path) => (path, true),
        InstallTarget::Installer(path) => (path, false),
    };
    prepare_install(bytes, destination, portable)?.finish(launch)
}

/// Verified fork release offered by the update panel.
#[derive(Clone)]
pub struct ReleaseUpdate {
    pub tag: String,
    pub name: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub portable_url: String,
    pub portable_name: String,
    pub portable_size: u64,
    pub portable_hash: String,
}
/// A validated installer awaiting an explicit install click.
#[derive(Clone)]
pub struct ReadyUpdate {
    pub path: PathBuf,
    pub sha256: String,
}
#[derive(serde::Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}
#[derive(serde::Deserialize)]
struct ReleaseMetadata {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<ReleaseAsset>,
}
fn release_client() -> Result<reqwest::Client, String> {
    crate::proxy::apply(
        reqwest::Client::builder()
            .user_agent("ctdozlite-update")
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let u = attempt.url();
                if attempt.previous().len() < 5
                    && u.scheme() == "https"
                    && u.username().is_empty()
                    && u.password().is_none()
                    && u.port_or_known_default() == Some(443)
                    && u.host_str().is_some_and(|host| {
                        crate::endpoints::RELEASE_REDIRECT_HOSTS.contains(&host)
                    })
                {
                    attempt.follow()
                } else {
                    attempt.error("Update redirect rejected")
                }
            })),
    )
    .map_err(|_| "Không tạo được kết nối cập nhật")?
    .build()
    .map_err(|_| "Không tạo được kết nối cập nhật".into())
}
fn version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.strip_prefix('v').unwrap_or(s);
    if let Some(serial) = s.strip_prefix("0.").and_then(|s| s.strip_suffix("-arm")) {
        if serial.is_empty() || !serial.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        return Some((0, 0, serial.parse().ok()?));
    }
    let mut p = s.split('.');
    let result = (
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
    );
    p.next().is_none().then_some(result)
}
fn choose_release(metadata: ReleaseMetadata, sums: &str) -> Result<Option<ReleaseUpdate>, String> {
    if metadata.draft || metadata.prerelease {
        return Ok(None);
    }
    let current = version(crate::PRODUCT_VERSION).ok_or("Phiên bản ứng dụng không hợp lệ")?;
    let next = version(&metadata.tag_name).ok_or("Phiên bản phát hành chưa được hỗ trợ")?;
    if next <= current {
        return Ok(None);
    }
    let portable = metadata
        .assets
        .iter()
        .find(|a| {
            a.name
                .to_ascii_lowercase()
                .ends_with("-portable-windows-arm64.exe")
        })
        .ok_or("Release thiếu ứng dụng ARM64 để xác minh kiến trúc")?;
    let portable_hash = sums
        .lines()
        .find_map(|line| {
            let mut p = line.split_whitespace();
            let h = p.next()?;
            let n = p.next()?.trim_start_matches('*');
            (n == portable.name && h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
                .then(|| h.to_ascii_lowercase())
        })
        .ok_or("Thiếu checksum ứng dụng ARM64")?;
    let portable = ReleaseAsset {
        name: portable.name.clone(),
        browser_download_url: portable.browser_download_url.clone(),
        size: portable.size,
    };
    if !portable
        .browser_download_url
        .starts_with(crate::endpoints::RELEASE_ASSET_PREFIX)
    {
        return Err("Nguồn ứng dụng ARM64 không hợp lệ".into());
    }
    let asset = metadata
        .assets
        .into_iter()
        .find(|a| {
            a.name
                .to_ascii_lowercase()
                .ends_with("-setup-windows-arm64.exe")
        })
        .ok_or("Release chưa có bộ cài Windows ARM64")?;
    validate_asset_name(&asset.name)?;
    if !asset
        .browser_download_url
        .starts_with(crate::endpoints::RELEASE_ASSET_PREFIX)
    {
        return Err("Nguồn cập nhật không thuộc repository fork".into());
    }
    let sha256 = sums
        .lines()
        .find_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            let name = parts.next()?.trim_start_matches('*');
            (name == asset.name && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
                .then(|| hash.to_ascii_lowercase())
        })
        .ok_or("Không có SHA-256 cho bộ cài ARM64")?;
    Ok(Some(ReleaseUpdate {
        tag: metadata.tag_name,
        name: asset.name,
        url: asset.browser_download_url,
        size: asset.size,
        sha256,
        portable_url: portable.browser_download_url,
        portable_name: portable.name,
        portable_size: portable.size,
        portable_hash,
    }))
}
/// Discover a newer stable ARM64 installer with a mandatory published checksum.
/// # Errors
/// Returns network, unsupported release or metadata validation failures.
pub async fn discover() -> Result<Option<ReleaseUpdate>, String> {
    let c = release_client()?;
    let r = c
        .get(crate::endpoints::RELEASE_LATEST)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| "Không kết nối được máy chủ cập nhật")?;
    if r.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !r.status().is_success() {
        return Err("Máy chủ cập nhật chưa trả metadata hợp lệ".into());
    }
    let bytes = crate::limits::response_bytes(r, 2 * 1024 * 1024)
        .await
        .map_err(|_| "Metadata cập nhật vượt giới hạn")?;
    let metadata: ReleaseMetadata =
        serde_json::from_slice(&bytes).map_err(|_| "Metadata cập nhật không hợp lệ")?;
    if metadata.draft
        || metadata.prerelease
        || version(&metadata.tag_name)
            .zip(version(crate::PRODUCT_VERSION))
            .is_some_and(|(next, current)| next <= current)
    {
        return Ok(None);
    }
    let sums = metadata
        .assets
        .iter()
        .find(|a| a.name == "SHA256SUMS-windows-arm64.txt")
        .ok_or("Release thiếu SHA256SUMS-windows-arm64.txt")?;
    if !sums
        .browser_download_url
        .starts_with(crate::endpoints::RELEASE_ASSET_PREFIX)
    {
        return Err("Nguồn checksum không hợp lệ".into());
    }
    let response = c
        .get(&sums.browser_download_url)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| "Không tải được checksum")?;
    if !response.status().is_success() {
        return Err("Không tải được checksum".into());
    }
    let bytes = crate::limits::response_bytes(response, 256 * 1024)
        .await
        .map_err(|_| "Checksum vượt giới hạn")?;
    let text = std::str::from_utf8(&bytes).map_err(|_| "Checksum không hợp lệ")?;
    choose_release(metadata, text)
}
/// Download to the private update directory, validate PE ARM64 and leave installation explicit.
/// # Errors
/// Rejects incorrect architecture/checksum and leaves the running binary untouched.
pub async fn prepare_update(
    release: ReleaseUpdate,
    progress: impl FnMut(u64, u64),
) -> Result<ReadyUpdate, String> {
    if !release
        .url
        .starts_with(crate::endpoints::RELEASE_ASSET_PREFIX)
    {
        return Err("Nguồn cập nhật không hợp lệ".into());
    }
    let portable = download_release(
        &release_client()?,
        &DownloadRequest {
            url: &release.portable_url,
            asset_name: &release.portable_name,
            expected_sha256: Some(&release.portable_hash),
            expected_size: Some(release.portable_size),
            limits: DownloadLimits::default(),
        },
        |_, _| {},
    )
    .await?;
    validate_arm64_pe(&portable)?;
    drop(portable);
    let bytes = download_release(
        &release_client()?,
        &DownloadRequest {
            url: &release.url,
            asset_name: &release.name,
            expected_sha256: Some(&release.sha256),
            expected_size: Some(release.size),
            limits: DownloadLimits::default(),
        },
        progress,
    )
    .await?;
    // Inno Setup may have an x86 bootstrapper even when its payload is ARM64.
    let hash = release.sha256;
    tokio::task::spawn_blocking(move || {
        let root = crate::storage::root().join("updates");
        crate::storage::private_dir(&root).map_err(|_| "Không tạo được thư mục cập nhật")?;
        let path = root.join(format!(
            "{}-{}",
            crate::chat::next_client_message_id(),
            release.name
        ));
        install_release(&bytes, &InstallTarget::Installer(path.clone()), |_| Ok(()))?;
        Ok(ReadyUpdate { path, sha256: hash })
    })
    .await
    .map_err(|_| "Không lưu được bộ cài cập nhật")?
}
fn validate_arm64_pe(bytes: &[u8]) -> Result<(), String> {
    if bytes.get(..2) != Some(b"MZ") {
        return Err("Bộ cài không phải PE".into());
    }
    let offset = bytes
        .get(0x3c..0x40)
        .and_then(|v| v.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or("Thiếu PE header")? as usize;
    if bytes.get(offset..offset.saturating_add(4)) != Some(b"PE\0\0")
        || bytes.get(offset.saturating_add(4)..offset.saturating_add(6)) != Some(&[0x64, 0xaa])
    {
        return Err("Bộ cài không dành cho Windows ARM64".into());
    }
    Ok(())
}
/// Launch only an unchanged validated installer inside the private update directory.
/// # Errors
/// Returns path, integrity or platform launcher failures without closing the app.
pub fn launch_update(ready: ReadyUpdate) -> Result<(), String> {
    let root = crate::storage::root()
        .join("updates")
        .canonicalize()
        .map_err(|_| "Không có thư mục cập nhật")?;
    let path = ready
        .path
        .canonicalize()
        .map_err(|_| "Không còn bộ cài cập nhật")?;
    if path.parent() != Some(root.as_path()) {
        return Err("Bộ cài nằm ngoài thư mục cập nhật".into());
    }
    let bytes =
        crate::limits::read_file(&path, 512 * 1024 * 1024).map_err(|_| "Không đọc được bộ cài")?;
    if !format!("{:x}", Sha256::digest(&bytes)).eq_ignore_ascii_case(&ready.sha256) {
        return Err("Bộ cài đã thay đổi; hãy tải lại".into());
    }
    if !bytes.starts_with(b"MZ") {
        return Err("Bộ cài không hợp lệ".into());
    }
    #[cfg(windows)]
    {
        std::process::Command::new(path)
            .spawn()
            .map_err(|_| "Không chạy được bộ cài")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err("Cài đặt cập nhật chỉ hỗ trợ Windows ARM64".into())
    }
}
#[cfg(test)]
mod release_tests {
    use super::*;
    #[test]
    fn wrong_architecture_is_rejected_and_arm64_is_accepted() {
        let mut b = vec![0; 128];
        b[..2].copy_from_slice(b"MZ");
        b[0x3c] = 64;
        b[64..68].copy_from_slice(b"PE\0\0");
        b[68..70].copy_from_slice(&[0x64, 0x86]);
        assert!(validate_arm64_pe(&b).is_err());
        b[68..70].copy_from_slice(&[0x64, 0xaa]);
        assert!(validate_arm64_pe(&b).is_ok());
    }
    #[test]
    fn asset_selection_never_uses_x64_or_unverified_assets() {
        let m = || ReleaseMetadata {
            tag_name: "v99.0.0".into(),
            draft: false,
            prerelease: false,
            assets: vec![ReleaseAsset {
                name: "ctdozlite-99.0.0-setup-windows-x86_64.exe".into(),
                browser_download_url: format!(
                    "{}v99.0.0/x.exe",
                    crate::endpoints::RELEASE_ASSET_PREFIX
                ),
                size: 1000000,
            }],
        };
        assert!(choose_release(m(), "").is_err());
        let mut r = m();
        r.assets[0].name = "ctdozlite-99.0.0-setup-windows-arm64.exe".into();
        assert!(choose_release(r, "").is_err());
    }
}

#[cfg(test)]
mod product_version_tests {
    use super::*;
    #[test]
    fn arm_release_serials_compare_with_legacy_versions() {
        assert!(version("v0.002-arm") < version(crate::PRODUCT_VERSION));
        assert_eq!(version(crate::PRODUCT_VERSION), Some((0, 0, 13)));
        assert!(version("v0.010-arm") > version("v0.009-arm"));
        assert_eq!(version("v0.0.3"), version("v0.003-arm"));
        for invalid in [
            "v0.003-arm-beta",
            "v0.003-x64",
            "v0.-arm",
            "vv0.003-arm",
            "0.0.3.1",
        ] {
            assert!(version(invalid).is_none(), "{invalid}");
        }
        assert!(choose_release(
            ReleaseMetadata {
                tag_name: crate::PRODUCT_VERSION.into(),
                draft: false,
                prerelease: false,
                assets: vec![]
            },
            ""
        )
        .unwrap()
        .is_none());
    }
}
