//! Native playback entry point; the worker owns downloads and the player owns COM.
use anyhow::Result;
use std::path::Path;
pub fn play(path: &Path, title: &str, active: impl Fn() -> bool) -> Result<()> {
    #[cfg(windows)]
    {
        native::run(path, title, active)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, title, active);
        anyhow::bail!("Trình phát video yêu cầu Windows ARM64")
    }
}
#[cfg(windows)]
#[path = "video_player_windows.rs"]
mod native;
