//! Screen region capture for the composer through the Windows snipping overlay.
//!
//! Windows ships the region picker as the `ms-screenclip:` URI (Snipping Tool). It puts the
//! capture on the clipboard, so the client waits for the clipboard sequence number to change
//! and then reads the image through the existing clipboard path. No screen pixels are read
//! by this process itself.
use anyhow::Result;

/// How long the composer waits for a capture before giving up.
pub const WAIT: std::time::Duration = std::time::Duration::from_secs(60);
/// Clipboard polling interval while waiting.
pub const POLL: std::time::Duration = std::time::Duration::from_millis(300);

/// Open the Windows region picker without a console window.
/// # Errors
/// Returns an error when the overlay cannot be started or on other platforms.
pub fn launch() -> Result<()> {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                hwnd: *mut c_void,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show: i32,
            ) -> isize;
        }
        let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
        let (open, uri) = (wide("open"), wide("ms-screenclip:"));
        // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the call.
        let code = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                open.as_ptr(),
                uri.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        // ShellExecuteW reports success with a value greater than 32.
        anyhow::ensure!(code > 32, "Không mở được công cụ chụp màn hình của Windows");
        Ok(())
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Chụp màn hình chỉ hỗ trợ trên Windows")
    }
}

/// Current clipboard sequence number; it changes whenever the clipboard content changes.
pub fn clipboard_sequence() -> Option<u32> {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        extern "system" {
            fn GetClipboardSequenceNumber() -> u32;
        }
        // SAFETY: no arguments; reads a per-session counter.
        let sequence = unsafe { GetClipboardSequenceNumber() };
        (sequence != 0).then_some(sequence)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Whether the clipboard changed since `before`.
pub fn changed(before: Option<u32>, now: Option<u32>) -> bool {
    matches!((before, now), (Some(a), Some(b)) if a != b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_new_sequence_counts_as_a_capture() {
        assert!(changed(Some(4), Some(5)));
        assert!(!changed(Some(4), Some(4)));
        assert!(!changed(None, Some(5)));
        assert!(!changed(Some(4), None));
    }
    #[cfg(not(windows))]
    #[test]
    fn other_platforms_report_unsupported() {
        assert!(launch().is_err());
        assert!(clipboard_sequence().is_none());
    }
}
