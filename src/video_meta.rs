//! Local video metadata for sending a video message like Zalo PC.
//!
//! Zalo PC sends a video as `/api/message/forward` with `msgType: 5` and
//! `msgInfo {videoUrl, thumbUrl, duration, width, height, fileSize, ...}` after uploading the
//! file (`video/file/upload`) and a poster (`video/thumb/upload`). Size and duration are read
//! from the MP4/MOV boxes here (pure Rust, bounded); the poster comes from Media Foundation
//! on Windows and is optional elsewhere.
use anyhow::Result;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Longest side of the poster uploaded with a video.
const POSTER_LONG_SIDE: u32 = 720;
/// Boxes are only walked this deep and this many times; the file is untrusted.
const MAX_BOXES: usize = 4096;

/// What the message needs to show a video before it is played.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

/// Extensions sent as a video message; anything else goes as a file.
pub fn is_video_message(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|e| matches!(e.as_str(), "mp4" | "mov" | "m4v"))
}

/// Read size and duration from an MP4/MOV file (`moov/mvhd`, video `trak/tkhd`).
pub fn probe(path: &Path) -> Result<VideoInfo> {
    let mut file = std::fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut info = VideoInfo::default();
    let mut budget = MAX_BOXES;
    walk(&mut file, 0, len, 0, &mut info, &mut budget)?;
    anyhow::ensure!(
        info.duration_ms > 0 || info.width > 0,
        "Không đọc được thông tin video"
    );
    Ok(info)
}

fn walk(
    file: &mut std::fs::File,
    start: u64,
    end: u64,
    depth: u8,
    info: &mut VideoInfo,
    budget: &mut usize,
) -> Result<()> {
    let mut pos = start;
    while pos + 8 <= end {
        anyhow::ensure!(*budget > 0, "Quá nhiều hộp trong tệp video");
        *budget -= 1;
        file.seek(SeekFrom::Start(pos))?;
        let mut head = [0u8; 8];
        file.read_exact(&mut head)?;
        let mut size = u64::from(u32::from_be_bytes([head[0], head[1], head[2], head[3]]));
        let kind = [head[4], head[5], head[6], head[7]];
        let mut header = 8;
        if size == 1 {
            let mut large = [0u8; 8];
            file.read_exact(&mut large)?;
            size = u64::from_be_bytes(large);
            header = 16;
        } else if size == 0 {
            size = end - pos;
        }
        if size < header || pos + size > end {
            break;
        }
        let body = pos + header;
        match &kind {
            b"moov" | b"trak" if depth < 4 => {
                walk(file, body, pos + size, depth + 1, info, budget)?
            }
            b"mvhd" => {
                let mut buf = [0u8; 32];
                let n = read_up_to(file, &mut buf)?;
                if let Some(ms) = mvhd_duration_ms(&buf[..n]) {
                    info.duration_ms = ms;
                }
            }
            b"tkhd" => {
                let mut buf = [0u8; 96];
                let n = read_up_to(file, &mut buf)?;
                if let Some((w, h)) = tkhd_size(&buf[..n]) {
                    if info.width == 0 && w > 0 && h > 0 {
                        info.width = w;
                        info.height = h;
                    }
                }
            }
            _ => {}
        }
        pos += size;
    }
    Ok(())
}

fn read_up_to(file: &mut std::fs::File, buf: &mut [u8]) -> Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match file.read(&mut buf[filled..])? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}
fn be64(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// `mvhd` body: version 0 → timescale @12, duration (u32) @16; version 1 → @20 / u64 @24.
fn mvhd_duration_ms(b: &[u8]) -> Option<u64> {
    let (scale, duration) = match *b.first()? {
        0 => (be32(b, 12)?, u64::from(be32(b, 16)?)),
        1 => (be32(b, 20)?, be64(b, 24)?),
        _ => return None,
    };
    (scale > 0 && duration < u64::MAX / 1000).then(|| duration * 1000 / u64::from(scale))
}

/// `tkhd` body: width/height are 16.16 fixed point at @76/@80 (v0) or @88/@92 (v1).
fn tkhd_size(b: &[u8]) -> Option<(u32, u32)> {
    let at = match *b.first()? {
        0 => 76,
        1 => 88,
        _ => return None,
    };
    Some((be32(b, at)? >> 16, be32(b, at + 4)? >> 16))
}

/// A JPEG poster of the first frame, long side `POSTER_LONG_SIDE`, when the platform can
/// decode the video. `None` means the message is sent without a poster.
pub fn poster_jpeg(path: &Path) -> Option<Vec<u8>> {
    #[cfg(windows)]
    {
        let (rgb, w, h) = crate::windows_video::poster_frame(path)
            .inspect_err(|e| crate::logger::log_error("video poster", &e.to_string()))
            .ok()?;
        let image = image::RgbImage::from_raw(w, h, rgb)?;
        let image = image::DynamicImage::ImageRgb8(image);
        let image = if w.max(h) > POSTER_LONG_SIDE {
            image.resize(
                POSTER_LONG_SIDE,
                POSTER_LONG_SIDE,
                image::imageops::FilterType::Triangle,
            )
        } else {
            image
        };
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
            .encode_image(&image.to_rgb8())
            .ok()?;
        Some(out)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, POSTER_LONG_SIDE);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        out
    }
    fn sample_mp4(version: u8) -> Vec<u8> {
        let mut mvhd = vec![version, 0, 0, 0];
        if version == 0 {
            mvhd.extend_from_slice(&[0; 8]);
            mvhd.extend_from_slice(&600u32.to_be_bytes());
            mvhd.extend_from_slice(&3300u32.to_be_bytes());
        } else {
            mvhd.extend_from_slice(&[0; 16]);
            mvhd.extend_from_slice(&600u32.to_be_bytes());
            mvhd.extend_from_slice(&3300u64.to_be_bytes());
        }
        mvhd.resize(100, 0);
        let mut audio_tkhd = vec![0u8; 84];
        audio_tkhd[0] = 0;
        let mut tkhd = vec![version, 0, 0, 0];
        tkhd.resize(if version == 0 { 76 } else { 88 }, 0);
        tkhd.extend_from_slice(&(1280u32 << 16).to_be_bytes());
        tkhd.extend_from_slice(&(720u32 << 16).to_be_bytes());
        let audio = mp4_box(b"trak", &mp4_box(b"tkhd", &audio_tkhd));
        let video = mp4_box(b"trak", &mp4_box(b"tkhd", &tkhd));
        let moov = mp4_box(b"moov", &[mp4_box(b"mvhd", &mvhd), audio, video].concat());
        [
            mp4_box(b"ftyp", b"isom\0\0\0\0"),
            mp4_box(b"mdat", &[1, 2, 3]),
            moov,
        ]
        .concat()
    }
    #[test]
    fn probe_reads_duration_and_video_track_size() {
        let root = std::env::temp_dir().join(format!(
            "video-meta-{}",
            crate::chat::next_client_message_id()
        ));
        std::fs::create_dir(&root).unwrap();
        for version in [0, 1] {
            let path = root.join(format!("v{version}.mp4"));
            std::fs::write(&path, sample_mp4(version)).unwrap();
            assert_eq!(
                probe(&path).unwrap(),
                VideoInfo {
                    width: 1280,
                    height: 720,
                    duration_ms: 5500
                }
            );
        }
        let broken = root.join("broken.mp4");
        std::fs::write(&broken, b"\0\0\0\x10moov\xff\xff").unwrap();
        assert!(probe(&broken).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn only_mp4_family_is_sent_as_video_message() {
        assert!(is_video_message(Path::new("a.MP4")));
        assert!(is_video_message(Path::new("a.mov")));
        assert!(!is_video_message(Path::new("a.mkv")));
        assert!(!is_video_message(Path::new("a.webm")));
    }
}
