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
    use std::sync::{
        mpsc::{sync_channel, SyncSender},
        OnceLock,
    };
    static PLAYER: OnceLock<Option<SyncSender<()>>> = OnceLock::new();
    let sender = PLAYER.get_or_init(|| {
        let (tx, rx) = sync_channel(1);
        match std::thread::Builder::new()
            .name("notification-audio".into())
            .spawn(move || {
                #[link(name = "winmm")]
                extern "system" {
                    fn mciSendStringW(
                        command: *const u16,
                        result: *mut u16,
                        len: u32,
                        callback: isize,
                    ) -> u32;
                }
                let command = |text: &str| {
                    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
                    // SAFETY: MCI copies the NUL-terminated buffer; no shell is involved.
                    unsafe { mciSendStringW(wide.as_ptr(), std::ptr::null_mut(), 0, 0) }
                };
                let mut opened = false;
                let mut last = None;
                while rx.recv().is_ok() {
                    if last.is_some_and(|at: std::time::Instant| {
                        at.elapsed() < std::time::Duration::from_millis(500)
                    }) {
                        continue;
                    }
                    let file = path();
                    let path = file.to_string_lossy();
                    if !file.is_file() || path.contains(['"', '\0']) {
                        continue;
                    }
                    if !opened {
                        let code = command(&format!(
                            "open \"{path}\" type mpegvideo alias ctdoz_notification"
                        ));
                        if code != 0 {
                            crate::logger::log_error(
                                "notification_audio",
                                &format!("open_failed code={code}"),
                            );
                            continue;
                        }
                        opened = true;
                    }
                    last = Some(std::time::Instant::now());
                    let code = command("play ctdoz_notification from 0 wait");
                    if code != 0 {
                        crate::logger::log_error(
                            "notification_audio",
                            &format!("play_failed code={code}"),
                        );
                        // Best-effort decoder cleanup allows the next notice to reopen it.
                        command("close ctdoz_notification");
                        opened = false;
                    }
                }
                if opened {
                    command("close ctdoz_notification");
                }
            }) {
            Ok(_) => Some(tx),
            Err(_) => {
                crate::logger::log_error("notification_audio", "Cannot start audio worker");
                None
            }
        }
    });
    if let Some(sender) = sender {
        let _ = sender.try_send(());
    }
}
#[cfg(not(windows))]
pub fn play() {}
