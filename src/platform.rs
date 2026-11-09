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

/// Open an already downloaded local file through the OS association.
///
/// A26 fix: Block executables, scripts, and shortcuts from auto-opening through
/// shell association. User must explicitly save and run them outside the app.
pub fn open_file(path: &std::path::Path) -> bool {
    if !path.is_file() {
        return false;
    }

    // A26: Gate executable/script/shortcut extensions (audit A26)
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        let ext_lower = ext.to_lowercase();
        let blocked = matches!(
            ext_lower.as_str(),
            "exe"
                | "com"
                | "bat"
                | "cmd"
                | "ps1"
                | "vbs"
                | "vbe"
                | "js"
                | "jse"
                | "wsf"
                | "wsh"
                | "msi"
                | "scr"
                | "pif"
                | "lnk"
                | "url"
                | "inf"
                | "reg"
        );
        if blocked {
            return false;
        }
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
