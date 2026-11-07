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
}
