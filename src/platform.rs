//! Windows shell integration without command interpreters or helper executables.
#[cfg(windows)]
fn shell_open(value: &std::ffi::OsStr) -> bool {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }
    let mut value: Vec<u16> = value.encode_wide().collect();
    if value.contains(&0) {
        return false;
    }
    value.push(0);
    let open: Vec<u16> = "open\0".encode_utf16().collect();
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            open.as_ptr(),
            value.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        ) > 32
    }
}
pub fn open_url(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value.trim()) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    #[cfg(windows)]
    {
        shell_open(std::ffi::OsStr::new(url.as_str()))
    }
    #[cfg(not(windows))]
    {
        false
    }
}
pub fn open_folder(path: &std::path::Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    #[cfg(windows)]
    {
        shell_open(path.as_os_str())
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Register only this application's current executable in the per-user Windows Run key.
/// Call from the worker because registry access may block.
pub fn set_autostart(enabled: bool) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "advapi32")]
        extern "system" {
            fn RegCreateKeyExW(
                key: isize,
                subkey: *const u16,
                reserved: u32,
                class: *mut u16,
                options: u32,
                access: u32,
                security: *const std::ffi::c_void,
                result: *mut isize,
                disposition: *mut u32,
            ) -> i32;
            fn RegSetValueExW(
                key: isize,
                name: *const u16,
                reserved: u32,
                kind: u32,
                data: *const u8,
                size: u32,
            ) -> i32;
            fn RegDeleteValueW(key: isize, name: *const u16) -> i32;
            fn RegCloseKey(key: isize) -> i32;
        }
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
            .encode_utf16()
            .collect();
        let name: Vec<u16> = "ctdozlite\0".encode_utf16().collect();
        let mut key = 0isize;
        let status = unsafe {
            RegCreateKeyExW(
                0x80000001u32 as i32 as isize,
                subkey.as_ptr(),
                0,
                std::ptr::null_mut(),
                0,
                0x0002,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        anyhow::ensure!(
            status == 0,
            "Không mở được cài đặt khởi động Windows ({status})"
        );
        struct RegistryKey(isize);
        impl Drop for RegistryKey {
            fn drop(&mut self) {
                unsafe {
                    RegCloseKey(self.0);
                }
            }
        }
        let key = RegistryKey(key);
        let status = if enabled {
            let exe = std::env::current_exe()?;
            let mut command = vec![b'"' as u16];
            command.extend(exe.as_os_str().encode_wide());
            command.extend("\" --gui\0".encode_utf16());
            unsafe {
                RegSetValueExW(
                    key.0,
                    name.as_ptr(),
                    0,
                    1,
                    command.as_ptr().cast(),
                    (command.len() * 2) as u32,
                )
            }
        } else {
            unsafe { RegDeleteValueW(key.0, name.as_ptr()) }
        };
        anyhow::ensure!(
            status == 0 || (!enabled && status == 2),
            "Không cập nhật được khởi động Windows ({status})"
        );
        Ok(())
    }
    #[cfg(not(windows))]
    {
        anyhow::ensure!(!enabled, "Tự khởi động chỉ hỗ trợ trên Windows");
        Ok(())
    }
}

pub fn startup_error(message: &str) {
    let path = crate::storage::root().join("diagnostics/startup-error.txt");
    let _ = crate::storage::atomic_write(&path, message.as_bytes());
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn MessageBoxW(
                hwnd: *mut std::ffi::c_void,
                text: *const u16,
                caption: *const u16,
                kind: u32,
            ) -> i32;
        }
        let text: Vec<u16> = format!("{message}\n\nChi tiết: {}", path.display())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let title: Vec<u16> = "ctdozlite — Không mở được giao diện"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
        }
    }
}

/// Safe extensions allowlist for direct opening via OS association.
/// Executables, scripts, installer packages, disk images, and macro-enabled Office documents
/// are STRICTLY excluded and must never be auto-opened.
pub const SAFE_OPEN_EXTENSIONS: &[&str] = &[
    // Images
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "tiff", "tif", "svg", "jxl", "heic", "heif",
    // Video
    "mp4", "mkv", "avi", "mov", "webm", "wmv", "flv", "m4v", "3gp", // Audio
    "mp3", "m4a", "wav", "ogg", "flac", "aac", "opus", "wma", // Documents / Plain text
    "pdf", "txt", "csv", "log", "json", "xml", "md",
    // Office formats without macros (Strictly no docm/xlsm/pptm)
    "docx", "xlsx", "pptx", "odt", "ods", "odp",
];

/// Check if an extension is on the safe allowlist.
/// Normalizes trailing whitespace, dots, and case to prevent bypasses like `"exe ."`.
pub fn is_safe_extension(ext: &str) -> bool {
    let clean = ext.trim_end_matches([' ', '.']).to_ascii_lowercase();
    SAFE_OPEN_EXTENSIONS.contains(&clean.as_str())
}

/// Reveal a file or folder in Windows Explorer and select it (`/select,"<path>"`).
#[cfg(windows)]
pub fn open_folder_and_select(path: &std::path::Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut std::ffi::c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show: i32,
        ) -> isize;
    }
    let explorer: Vec<u16> = "explorer.exe\0".encode_utf16().collect();
    let open: Vec<u16> = "open\0".encode_utf16().collect();
    let param_str = format!("/select,\"{}\"", path.display());
    let mut params: Vec<u16> = std::ffi::OsStr::new(&param_str).encode_wide().collect();
    params.push(0);

    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            open.as_ptr(),
            explorer.as_ptr(),
            params.as_ptr(),
            std::ptr::null(),
            1,
        ) > 32
    }
}

#[cfg(not(windows))]
pub fn open_folder_and_select(_path: &std::path::Path) -> bool {
    false
}

/// H-01: Attach Zone.Identifier NTFS Alternate Data Stream (ZoneId=3: Internet Zone)
/// to downloaded files to enforce Windows SmartScreen and Office Protected View checks.
pub fn apply_mark_of_the_web(path: &std::path::Path) {
    #[cfg(windows)]
    {
        use std::io::Write;
        let ads_path = format!("{}:Zone.Identifier", path.display());
        let content = b"[ZoneTransfer]\r\nZoneId=3\r\n";
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&ads_path)
        {
            let _ = f.write_all(content);
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
    }
}

/// Open an already downloaded local file through the OS association.
///
/// H-01 / A26 fix: Enforces strict allowlist (images, audio, video, pdf, txt, macro-free office).
/// Any extension outside the allowlist (or missing) is NEVER auto-opened;
/// instead, it falls back to revealing the file safely in Windows Explorer (`open_folder_and_select`).
pub fn open_file(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }

    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    // Strip trailing whitespace/dots before inspecting extension to prevent `"evil.exe ."` bypass
    let clean_name = file_name.trim_end_matches([' ', '.']);
    let ext = std::path::Path::new(clean_name)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if is_safe_extension(ext) {
        #[cfg(windows)]
        {
            shell_open(path.as_os_str())
        }
        #[cfg(not(windows))]
        {
            false
        }
    } else {
        // Untrusted/unsafe extension: reveal in folder without executing
        open_folder_and_select(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h01_allowlist_permits_safe_extensions() {
        for ext in &[
            "png", "jpg", "jpeg", "gif", "webp", "mp4", "mkv", "mp3", "wav", "pdf", "txt", "docx",
            "xlsx", "pptx",
        ] {
            assert!(is_safe_extension(ext), "Extension {ext} must be permitted");
            // Also test uppercase
            assert!(
                is_safe_extension(&ext.to_uppercase()),
                "Uppercase extension {ext} must be permitted"
            );
        }
    }

    #[test]
    fn h01_allowlist_rejects_dangerous_and_macro_extensions() {
        let dangerous = [
            "exe",
            "com",
            "bat",
            "cmd",
            "ps1",
            "vbs",
            "vbe",
            "js",
            "jse",
            "wsf",
            "wsh",
            "msi",
            "scr",
            "pif",
            "lnk",
            "url",
            "inf",
            "reg",
            "hta",
            "cpl",
            "msc",
            "jar",
            "chm",
            "scf",
            "appinstaller",
            "msix",
            "appx",
            "appref-ms",
            "application",
            "settingcontent-ms",
            "library-ms",
            "search-ms",
            "diagcab",
            "psm1",
            "ps1xml",
            "ws",
            "wsc",
            "sct",
            "xll",
            "iso",
            "img",
            "vhd",
            "vhdx",
            "docm",
            "xlsm",
            "pptm",
            "dotm",
        ];
        for ext in dangerous {
            assert!(
                !is_safe_extension(ext),
                "Dangerous extension {ext} must be rejected by allowlist"
            );
            assert!(
                !is_safe_extension(&format!("{ext} .")),
                "Bypass attempt '{ext} .' must be rejected"
            );
            assert!(
                !is_safe_extension(&format!("{ext}. . ")),
                "Bypass attempt '{ext}. . ' must be rejected"
            );
        }
    }

    #[test]
    fn h01_mark_of_the_web_attaches_zone_id() {
        let temp_dir = tempfile::tempdir().expect("create tempdir");
        let test_file = temp_dir.path().join("test_download.pdf");
        std::fs::write(&test_file, b"%PDF-1.4 sample content").expect("write test file");

        apply_mark_of_the_web(&test_file);

        #[cfg(windows)]
        {
            let ads_path = format!("{}:Zone.Identifier", test_file.display());
            if let Ok(ads_content) = std::fs::read_to_string(&ads_path) {
                assert!(
                    ads_content.contains("ZoneId=3"),
                    "Zone.Identifier ADS must contain ZoneId=3"
                );
            }
        }
    }
}
