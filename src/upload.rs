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
