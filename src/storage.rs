//! User-scoped paths and crash-safe persistence. Never depend on the launch directory.
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

static ROOT: OnceLock<PathBuf> = OnceLock::new();
pub(crate) static SESSION_IO: Mutex<()> = Mutex::new(());

fn default_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("CTDOZ_DATA_DIR") {
        let path = PathBuf::from(path);
        anyhow::ensure!(
            path.is_absolute(),
            "CTDOZ_DATA_DIR must be an absolute path"
        );
        return Ok(path);
    }
    #[cfg(windows)]
    return Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is unavailable")?,
    )
    .join("ctdozlite"));
    #[cfg(target_os = "macos")]
    return Ok(
        PathBuf::from(std::env::var_os("HOME").context("HOME is unavailable")?)
            .join("Library/Application Support/ctdozlite"),
    );
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .context("No user data directory is available")?;
        Ok(base.join("ctdozlite"))
    }
}

/// Longest accepted `--profile` name.
const PROFILE_MAX_LEN: usize = 32;

/// Validate a `--profile` name so it maps to exactly one directory on every platform.
///
/// # Errors
/// Rejects empty or overlong names, characters other than ASCII letters, digits, `-` and `_`,
/// and Windows reserved device names. Names are rejected rather than silently rewritten so two
/// different inputs can never share one profile.
pub fn validate_profile(name: &str) -> Result<String> {
    anyhow::ensure!(
        !name.is_empty() && name.len() <= PROFILE_MAX_LEN,
        "Tên hồ sơ phải dài 1–{PROFILE_MAX_LEN} ký tự"
    );
    anyhow::ensure!(
        name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Tên hồ sơ chỉ được dùng chữ, số, '-' hoặc '_'"
    );
    let upper = name.to_ascii_uppercase();
    let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit());
    anyhow::ensure!(!reserved, "Tên hồ sơ trùng tên thiết bị dành riêng của Windows");
    // Case-insensitive file systems would otherwise merge `Work` and `work`.
    Ok(name.to_ascii_lowercase())
}

/// Read `--profile NAME` or `--profile=NAME` from command-line arguments.
///
/// # Errors
/// Fails when the flag is repeated, has no value, or the name is invalid.
pub fn profile_from_args(args: &[String]) -> Result<Option<String>> {
    let mut found = None;
    let mut i = 0;
    while i < args.len() {
        let value = if args[i] == "--profile" {
            i += 1;
            Some(
                args.get(i)
                    .filter(|v| !v.starts_with('-'))
                    .context("--profile cần tên hồ sơ, ví dụ: --profile cong-viec")?
                    .as_str(),
            )
        } else {
            args[i].strip_prefix("--profile=")
        };
        if let Some(value) = value {
            anyhow::ensure!(found.is_none(), "--profile chỉ được dùng một lần");
            found = Some(validate_profile(value.trim())?);
        }
        i += 1;
    }
    Ok(found)
}

fn profile_root(base: PathBuf, profile: Option<&str>) -> PathBuf {
    match profile {
        Some(name) => base.join("profiles").join(name),
        None => base,
    }
}

pub fn init(portable: bool) -> Result<()> {
    init_profile(portable, None)
}

/// Select the data directory once; a profile gets its own sessions, history, settings and
/// instance lock under `<root>/profiles/<name>`.
///
/// # Errors
/// Fails when the directory cannot be created or the root was already initialized.
pub fn init_profile(portable: bool, profile: Option<&str>) -> Result<()> {
    let base = if portable {
        std::env::current_exe()?
            .parent()
            .context("Executable has no directory")?
            .join("data")
    } else {
        default_root()?
    };
    if let Some(name) = profile {
        anyhow::ensure!(validate_profile(name)? == name, "Invalid profile name");
    }
    let path = profile_root(base, profile);
    private_dir(&path)?;
    ROOT.set(path)
        .map_err(|_| anyhow::anyhow!("Data directory is already initialized"))?;
    Ok(())
}

pub fn root() -> &'static Path {
    ROOT.get_or_init(|| default_root().expect("Cannot resolve user data directory"))
        .as_path()
}

pub fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn account_file(account: &str, name: &str) -> PathBuf {
    // Hash the account identifier: no path traversal, raw identifiers or ambiguous sanitizing.
    let key = format!("{:x}", Sha256::digest(account.as_bytes()));
    root().join("accounts").join(key).join(name)
}

pub fn session_path(path: &str) -> PathBuf {
    if path == "session.json" {
        root().join("session.dat")
    } else {
        PathBuf::from(path)
    }
}

pub fn settings_path() -> PathBuf {
    root().join("app_settings.json")
}
pub fn cache_dir() -> PathBuf {
    root().join("media-cache")
}

pub fn remember_login() -> Result<bool> {
    match fs::read(settings_path()) {
        Ok(bytes) => Ok(serde_json::from_slice::<serde_json::Value>(&bytes)?
            .get("remember_login")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.into()),
    }
}

pub fn save_settings(value: &impl serde::Serialize) -> Result<()> {
    let _lock = SESSION_IO
        .lock()
        .map_err(|_| anyhow::anyhow!("Session persistence lock poisoned"))?;
    let bytes = serde_json::to_vec_pretty(value)?;
    atomic_write(&settings_path(), &bytes)?;
    if !remember_login()? {
        remove_session_unlocked()?;
    }
    Ok(())
}

pub(crate) fn remove_session_unlocked() -> Result<()> {
    match fs::remove_file(session_path("session.json")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

pub fn remove_session() -> Result<()> {
    let _lock = SESSION_IO
        .lock()
        .map_err(|_| anyhow::anyhow!("Session persistence lock poisoned"))?;
    remove_session_unlocked()
}

pub fn lock_instance() -> Result<File> {
    private_dir(root())?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root().join("instance.lock"))?;
    file.try_lock()
        .context("ctdozlite is already running for this data directory")?;
    Ok(file)
}

/// A sibling temporary file plus atomic replace preserves the previous complete snapshot on failure.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    private_dir(parent)?;
    let mut random = [0u8; 16];
    crate::random::fill(&mut random)?;
    let name = random
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let temporary = parent.join(format!(".ctdoz-{name}.tmp"));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        replace(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(windows))]
fn replace(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to)?;
    Ok(())
}

#[cfg(windows)]
fn replace(from: &Path, to: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

#[cfg(windows)]
mod dpapi {
    use super::*;
    #[repr(C)]
    struct Blob {
        len: u32,
        data: *mut u8,
    }
    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            input: *const Blob,
            description: *const u16,
            entropy: *const Blob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
        fn CryptUnprotectData(
            input: *const Blob,
            description: *mut *mut u16,
            entropy: *const Blob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            output: *mut Blob,
        ) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }
    const PREFIX: &[u8] = b"CTDOZ-DPAPI-1\n";
    fn transform(bytes: &[u8], protect: bool) -> Result<Vec<u8>> {
        let input = Blob {
            len: bytes.len().try_into()?,
            data: bytes.as_ptr() as *mut u8,
        };
        let mut output = Blob {
            len: 0,
            data: std::ptr::null_mut(),
        };
        // UI forbidden, current-user protection (never CRYPTPROTECT_LOCAL_MACHINE).
        let ok = unsafe {
            if protect {
                CryptProtectData(
                    &input,
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    1,
                    &mut output,
                )
            } else {
                CryptUnprotectData(
                    &input,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    1,
                    &mut output,
                )
            }
        };
        if ok == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let result = if output.len == 0 {
            Vec::new()
        } else {
            anyhow::ensure!(!output.data.is_null(), "DPAPI returned a null buffer");
            unsafe { std::slice::from_raw_parts(output.data, output.len as usize).to_vec() }
        };
        unsafe {
            LocalFree(output.data.cast());
        }
        Ok(result)
    }
    pub fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
        let mut out = PREFIX.to_vec();
        out.extend(transform(bytes, true)?);
        Ok(out)
    }
    pub fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
        let encrypted = bytes
            .strip_prefix(PREFIX)
            .context("Unprotected legacy session: sign in again to create a protected session")?;
        transform(encrypted, false)
    }
}
#[cfg(windows)]
pub use dpapi::{protect, unprotect};
#[cfg(not(windows))]
pub fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(bytes.to_vec())
}
#[cfg(not(windows))]
pub fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_names_are_strict_and_case_folded_when_parsed() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(profile_from_args(&args(&["--gui"])).unwrap(), None);
        assert_eq!(
            profile_from_args(&args(&["--profile", "Cong_Viec-2"])).unwrap(),
            Some("cong_viec-2".into())
        );
        assert_eq!(
            profile_from_args(&args(&["--profile=work"])).unwrap(),
            Some("work".into())
        );
        for bad in [
            &["--profile"][..],
            &["--profile", "--gui"],
            &["--profile=../x"],
            &["--profile=a b"],
            &["--profile=nul"],
            &["--profile=COM1"],
            &["--profile=tài-khoản"],
            &["--profile=a", "--profile=b"],
        ] {
            assert!(profile_from_args(&args(bad)).is_err(), "{bad:?}");
        }
        assert!(validate_profile(&"x".repeat(33)).is_err());
        assert!(validate_profile("COM").is_ok());
    }
    #[test]
    fn profile_root_is_nested_under_profiles_when_selected() {
        let base = PathBuf::from("/data/ctdozlite");
        assert_eq!(profile_root(base.clone(), None), base);
        assert_eq!(
            profile_root(base.clone(), Some("work")),
            base.join("profiles").join("work")
        );
    }
    #[test]
    fn account_paths_cannot_escape_or_collide() {
        let a = account_file("../alice", "history.db");
        let b = account_file("alice", "history.db");
        assert_ne!(a, b);
        assert!(a.starts_with(root().join("accounts")));
        assert_eq!(
            a.parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .len(),
            64
        );
    }
    #[test]
    fn atomic_replace_preserves_complete_snapshots() {
        let directory = std::env::temp_dir().join(format!(
            "ctdoz-storage-{}",
            crate::param_cipher::random_hex(24)
        ));
        let path = directory.join("snapshot.json");
        atomic_write(&path, b"first").unwrap();
        atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrip_and_tampering() {
        assert!(unprotect(&protect(b"").unwrap()).unwrap().is_empty());
        let secret = b"session-token-test-fixture";
        let mut encrypted = protect(secret).unwrap();
        assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
        assert_eq!(unprotect(&encrypted).unwrap(), secret);
        let last = encrypted.len() - 1;
        encrypted[last] ^= 1;
        assert!(unprotect(&encrypted).is_err());
        assert!(unprotect(secret).is_err());
    }
}
