//! Cache the public Zalo notification asset once; never fetch it per message.
use anyhow::{ensure, Result};
fn path() -> std::path::PathBuf {
    crate::storage::root().join("public-assets/noti.mp3")
}
pub async fn prepare() -> Result<()> {
    let target = path();
    let check = target.clone();
    if tokio::task::spawn_blocking(move || {
        valid(&crate::limits::read_file(&check, 1024 * 1024).unwrap_or_default())
    })
    .await?
    {
        return Ok(());
    }
    let response = crate::media_download::response(
        crate::endpoints::NOTIFICATION_SOUND,
        std::time::Duration::from_secs(20),
    )
    .await?;
    let bytes = crate::limits::response_bytes(response, 1024 * 1024).await?;
    ensure!(valid(&bytes), "Âm báo Zalo không hợp lệ");
    tokio::task::spawn_blocking(move || crate::storage::atomic_write(&target, &bytes)).await??;
    Ok(())
}
fn valid(bytes: &[u8]) -> bool {
    bytes.len() > 32 && (bytes.starts_with(b"ID3") || (bytes[0] == 255 && bytes[1] & 0xe0 == 0xe0))
}
#[cfg(windows)]
pub fn play() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static PLAYING: AtomicBool = AtomicBool::new(false);
    if PLAYING.swap(true, Ordering::AcqRel) {
        return;
    }
    if std::thread::Builder::new()
        .name("notification-audio".into())
        .spawn(|| {
            struct Guard;
            impl Drop for Guard {
                fn drop(&mut self) {
                    PLAYING.store(false, Ordering::Release);
                }
            }
            let _guard = Guard;
            let path = path();
            if !path.is_file() {
                return;
            }
            let path = path.to_string_lossy();
            if path.contains(['"', '\0']) {
                return;
            }
            #[link(name = "winmm")]
            extern "system" {
                fn mciSendStringW(
                    command: *const u16,
                    result: *mut u16,
                    len: u32,
                    callback: isize,
                ) -> u32;
            }
            let command = |s: &str| {
                let s: Vec<u16> = s.encode_utf16().chain(Some(0)).collect(); // SAFETY: MCI copies this NUL-terminated command; no shell is involved.
                unsafe { mciSendStringW(s.as_ptr(), std::ptr::null_mut(), 0, 0) }
            };
            if command(&format!(
                "open \"{path}\" type mpegvideo alias ctdoz_notification"
            )) == 0
            {
                let _ = command("play ctdoz_notification from 0 wait");
                let _ = command("close ctdoz_notification");
            }
        })
        .is_err()
    {
        PLAYING.store(false, Ordering::Release);
    }
}
#[cfg(not(windows))]
pub fn play() {}
