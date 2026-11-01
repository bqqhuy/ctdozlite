//! Resolve an explicit, architecture-checked FFmpeg instead of silently using x64 emulation.
use anyhow::{Context, Result};
use std::io::Read;
#[cfg(any(test, all(windows, target_arch = "aarch64")))]
use std::io::{Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(any(test, all(windows, target_arch = "aarch64")))]
fn machine(mut file: impl Read + Seek) -> Result<u16> {
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos)?;
    anyhow::ensure!(&dos[..2] == b"MZ", "Not a PE executable");
    let offset = u32::from_le_bytes(dos[60..64].try_into()?);
    anyhow::ensure!(
        (64..=1024 * 1024).contains(&offset),
        "Invalid PE header offset"
    );
    file.seek(SeekFrom::Start(offset as u64))?;
    let mut header = [0; 6];
    file.read_exact(&mut header)?;
    anyhow::ensure!(&header[..4] == b"PE\0\0", "Invalid PE signature");
    Ok(u16::from_le_bytes(header[4..6].try_into()?))
}

fn resolve() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("CTDOZ_FFMPEG") {
        let path = PathBuf::from(path);
        anyhow::ensure!(
            path.is_absolute() && path.is_file(),
            "CTDOZ_FFMPEG must name an existing absolute file path"
        );
        return Ok(path);
    }
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    let executable = std::env::current_exe()?;
    if let Some(parent) = executable.parent() {
        let bundled = parent.join("ffmpeg-arm64").join(name);
        if bundled.is_file() {
            return Ok(bundled);
        }
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        // Do not implicitly execute a program from the current working directory.
        if !directory.is_absolute() {
            continue;
        }
        let path = directory.join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    anyhow::bail!("Video requires FFmpeg. Set CTDOZ_FFMPEG to an absolute path to a native ARM64 build with dshow, libx264 and H.264 decoding.")
}

fn check_architecture(path: &Path) -> Result<()> {
    #[cfg(all(windows, target_arch = "aarch64"))]
    anyhow::ensure!(
        machine(std::fs::File::open(path)?)? == 0xAA64,
        "FFmpeg must be native ARM64 (PE machine 0xAA64); x64 emulation is disabled for video"
    );
    let _ = path;
    Ok(())
}

pub fn command() -> Result<Command> {
    let path = resolve()?;
    check_architecture(&path)?;
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd.stdin(Stdio::null());
    Ok(cmd)
}

fn listing(path: &Path, argument: &str) -> Result<String> {
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let mut child = cmd
        .args(["-hide_banner", argument])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().context("FFmpeg stdout unavailable")?;
    let stderr = child.stderr.take().context("FFmpeg stderr unavailable")?;
    fn read(pipe: impl Read) -> Vec<u8> {
        let mut out = Vec::new();
        let _ = pipe.take(256 * 1024).read_to_end(&mut out);
        out
    }
    let out = std::thread::spawn(move || read(stdout));
    let err = std::thread::spawn(move || read(stderr));
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if start.elapsed() > std::time::Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let mut bytes = out.join().unwrap_or_default();
    bytes.extend(err.join().unwrap_or_default());
    anyhow::ensure!(
        status
            .context("FFmpeg capability check timed out")?
            .success(),
        "FFmpeg capability check failed"
    );
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(windows)]
pub fn readiness() -> Result<PathBuf> {
    crate::windows_video::readiness()?;
    Ok(PathBuf::from("Windows Media Foundation (native)"))
}

#[cfg(not(windows))]
pub fn readiness() -> Result<PathBuf> {
    let path = resolve()?;
    check_architecture(&path)?;
    let encoders = listing(&path, "-encoders")?;
    let decoders = listing(&path, "-decoders")?;
    anyhow::ensure!(
        encoders.split_whitespace().any(|word| word == "libx264"),
        "FFmpeg lacks the libx264 encoder"
    );
    anyhow::ensure!(
        decoders.split_whitespace().any(|word| word == "h264"),
        "FFmpeg lacks the H.264 decoder"
    );
    let filters = listing(&path, "-filters")?;
    for name in ["fps", "scale", "pad", "format"] {
        anyhow::ensure!(
            filters.split_whitespace().any(|word| word == name),
            "FFmpeg lacks the {name} filter"
        );
    }
    #[cfg(windows)]
    anyhow::ensure!(
        listing(&path, "-devices")?
            .split_whitespace()
            .any(|word| word == "dshow"),
        "FFmpeg lacks DirectShow (dshow) camera capture"
    );
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_arm64_and_x64_without_executing_them() {
        for expected in [0xAA64u16, 0x8664] {
            let mut pe = vec![0; 128];
            pe[..2].copy_from_slice(b"MZ");
            pe[60..64].copy_from_slice(&64u32.to_le_bytes());
            pe[64..68].copy_from_slice(b"PE\0\0");
            pe[68..70].copy_from_slice(&expected.to_le_bytes());
            assert_eq!(machine(std::io::Cursor::new(&pe)).unwrap(), expected);
            pe[64] = 0;
            assert!(machine(std::io::Cursor::new(&pe)).is_err());
        }
        assert!(machine(std::io::Cursor::new(b"truncated")).is_err());
    }
}
