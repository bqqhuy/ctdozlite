//! Native Windows video capability check. No external process or emulated codec.
use anyhow::Result;
pub fn readiness() -> Result<&'static str> {
    #[cfg(windows)]
    {
        crate::windows_video::readiness()?;
        Ok("Windows Media Foundation (native ARM64)")
    }
    #[cfg(not(windows))]
    anyhow::bail!("Video requires Windows 11 ARM64")
}
