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
