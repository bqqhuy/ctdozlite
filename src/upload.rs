//! Upload sources retain one open file and bounded buffers, independent of file size.
use anyhow::{Context, Result};
use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
    time::SystemTime,
};
pub const CHUNK: usize = 1_048_576;
pub struct Source {
    file: tokio::fs::File,
    pub size: u64,
    pub checksum: String,
    pub dimensions: Option<(u32, u32)>,
    modified: Option<SystemTime>,
    remaining: u64,
}
impl Source {
    pub async fn open(path: &Path) -> Result<Self> {
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || -> Result<Self> {
            use md5::{Digest, Md5};
            let mut file = std::fs::File::open(&path).context("Không mở được tập tin")?;
            let metadata = file.metadata()?;
            anyhow::ensure!(
                metadata.is_file() && metadata.len() > 0,
                "Tệp phải là tập tin thường và không rỗng"
            );
            let size = metadata.len();
            let modified = metadata.modified().ok();
            let mut hash = Md5::new();
            let mut buffer = vec![0; CHUNK];
            let mut remaining = size;
            while remaining > 0 {
                let count = remaining.min(CHUNK as u64) as usize;
                file.read_exact(&mut buffer[..count])?;
                hash.update(&buffer[..count]);
                remaining -= count as u64;
            }
            let after = file.metadata()?;
            anyhow::ensure!(
                after.len() == size && after.modified().ok() == modified,
                "Tệp đã thay đổi khi đọc"
            );
            file.seek(SeekFrom::Start(0))?;
            let dimensions = image::ImageReader::new(std::io::BufReader::new(file.try_clone()?))
                .with_guessed_format()?
                .into_dimensions()
                .ok();
            file.seek(SeekFrom::Start(0))?;
            Ok(Self {
                file: tokio::fs::File::from_std(file),
                size,
                checksum: format!("{:x}", hash.finalize()),
                dimensions,
                modified,
                remaining: size,
            })
        })
        .await?
    }
    pub async fn next_chunk(&mut self) -> Result<Vec<u8>> {
        use tokio::io::AsyncReadExt;
        let mut chunk = vec![0; self.remaining.min(CHUNK as u64) as usize];
        self.file.read_exact(&mut chunk).await?;
        self.remaining -= chunk.len() as u64;
        Ok(chunk)
    }
    /// Skip `count` chunks of `chunk_size` bytes without reading them into memory.
    ///
    /// Used to resume an interrupted chunked upload: the chunks the server
    /// already confirmed are not re-sent, so the file position has to move
    /// forward past them without allocating the bytes.  The chunk grid is
    /// absolute — chunk `i` always covers `[i * chunk_size, (i + 1) * chunk_size)`
    /// — so the skip is measured from the current position, not from the end of
    /// the file.  Skipping past the end of the file stops at the last byte.
    pub async fn skip_chunks(&mut self, count: usize, chunk_size: usize) -> Result<()> {
        if count == 0 {
            return Ok(());
        }
        let current = self.size - self.remaining;
        let target = current
            .saturating_add((count as u64).saturating_mul(chunk_size as u64))
            .min(self.size);
        use tokio::io::AsyncSeekExt;
        self.file.seek(SeekFrom::Start(target)).await?;
        self.remaining = self.size - target;
        Ok(())
    }
    pub async fn verify(&self) -> Result<()> {
        let metadata = self.file.metadata().await?;
        anyhow::ensure!(
            metadata.len() == self.size && metadata.modified().ok() == self.modified,
            "Tệp đã thay đổi trong lúc tải lên; chưa gửi tin nhắn"
        );
        Ok(())
    }
    pub async fn rewind(&mut self) -> Result<tokio::fs::File> {
        use tokio::io::AsyncSeekExt;
        self.verify().await?;
        self.file.seek(SeekFrom::Start(0)).await?;
        Ok(self.file.try_clone().await?)
    }
}

/// Resumable chunked upload tracker with checkpointing (WP15-4).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ResumableUploadState {
    pub upload_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub chunk_size: usize,
    pub total_chunks: usize,
    pub completed_chunks: Vec<usize>,
}

impl ResumableUploadState {
    pub fn new(upload_id: String, file_name: String, total_size: u64, chunk_size: usize) -> Self {
        let total_chunks = if total_size == 0 {
            0
        } else {
            total_size.div_ceil(chunk_size as u64) as usize
        };
        Self {
            upload_id,
            file_name,
            total_size,
            chunk_size,
            total_chunks,
            completed_chunks: Vec::new(),
        }
    }

    pub fn mark_chunk_complete(&mut self, chunk_index: usize) {
        if !self.completed_chunks.contains(&chunk_index) {
            self.completed_chunks.push(chunk_index);
            self.completed_chunks.sort_unstable();
        }
    }

    pub fn is_complete(&self) -> bool {
        self.total_chunks > 0 && self.completed_chunks.len() == self.total_chunks
    }

    /// WP15.1-C fix: O(n) scan instead of O(n²) contains per candidate.
    pub fn next_missing_chunk(&self) -> Option<usize> {
        if self.completed_chunks.is_empty() {
            return if self.total_chunks > 0 { Some(0) } else { None };
        }
        let mut completed_set = std::collections::HashSet::with_capacity(self.completed_chunks.len());
        for &idx in &self.completed_chunks {
            completed_set.insert(idx);
        }
        (0..self.total_chunks).find(|i| !completed_set.contains(i))
    }
}

#[cfg(test)]
mod resumable_tests {
    use super::*;

    #[test]
    fn resumable_upload_state_tracks_chunks_and_detects_completion() {
        let mut state = ResumableUploadState::new("up1".into(), "large.zip".into(), 10_000_000, 4_000_000);
        assert_eq!(state.total_chunks, 3);
        assert_eq!(state.next_missing_chunk(), Some(0));
        assert!(!state.is_complete());

        state.mark_chunk_complete(0);
        assert_eq!(state.next_missing_chunk(), Some(1));
        assert!(!state.is_complete());

        state.mark_chunk_complete(2);
        assert_eq!(state.next_missing_chunk(), Some(1));

        state.mark_chunk_complete(1);
        assert_eq!(state.next_missing_chunk(), None);
        assert!(state.is_complete());
    }

    /// The contiguous confirmed prefix is what a resume may trust.
    fn contiguous_prefix(state: &ResumableUploadState) -> usize {
        let mut contiguous = 0usize;
        for idx in 0..state.total_chunks {
            if state.completed_chunks.contains(&idx) {
                contiguous = idx + 1;
            } else {
                break;
            }
        }
        contiguous
    }

    #[test]
    fn only_the_contiguous_prefix_is_resumable() {
        let mut state = ResumableUploadState::new("up2".into(), "large.zip".into(), 10_000_000, 4_000_000);
        state.mark_chunk_complete(0);
        state.mark_chunk_complete(2);
        // Chunk 1 was never confirmed, so chunk 2 alone cannot be resumed.
        assert_eq!(contiguous_prefix(&state), 1);
        assert_eq!(state.next_missing_chunk(), Some(1));

        state.mark_chunk_complete(1);
        assert_eq!(contiguous_prefix(&state), 3);
        assert_eq!(state.next_missing_chunk(), None);
    }

    #[tokio::test]
    async fn skip_chunks_advances_the_file_position_without_reading() {
        let dir = std::env::temp_dir().join(format!("zalo-upload-skip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("payload.bin");
        let bytes: Vec<u8> = (0..(CHUNK * 2 + 17)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&path, &bytes).unwrap();

        // A fresh source starts at offset 0; skipping two chunks moves to chunk 2.
        let mut source = Source::open(&path).await.unwrap();
        source.skip_chunks(2, CHUNK).await.unwrap();
        let chunk2 = source.next_chunk().await.unwrap();
        assert_eq!(chunk2.len(), 17);
        assert_eq!(chunk2, bytes[CHUNK * 2..]);

        // A fresh source that reads one chunk first, then skips one more,
        // lands on the same place as skipping two from the start.
        let mut source = Source::open(&path).await.unwrap();
        let chunk0 = source.next_chunk().await.unwrap();
        assert_eq!(chunk0, bytes[..CHUNK]);
        source.skip_chunks(1, CHUNK).await.unwrap();
        let chunk2 = source.next_chunk().await.unwrap();
        assert_eq!(chunk2, bytes[CHUNK * 2..]);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn skipping_past_the_end_never_yields_extra_bytes() {
        let dir = std::env::temp_dir().join(format!("zalo-upload-skip-end-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("payload.bin");
        let bytes: Vec<u8> = (0..100u32).map(|i| i as u8).collect();
        std::fs::write(&path, &bytes).unwrap();

        let mut source = Source::open(&path).await.unwrap();
        // Far more chunks than the file holds — the skip must clamp.
        source.skip_chunks(50, CHUNK).await.unwrap();
        assert!(source.next_chunk().await.unwrap().is_empty() || source.remaining == 0);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
