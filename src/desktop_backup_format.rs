//! Bounded Desktop V2 format reader adapted from upstream PR96 (MIT/Apache-2.0).
//!
//! The cipher parameters match the installed Zalo Desktop 26.10.20 backup code.

use aes::Aes256;
use anyhow::{bail, Result};
use cipher::{
    block_padding::{NoPadding, Pkcs7},
    BlockDecryptMut, KeyIvInit,
};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::io::Read;

/// Verify a V2 backup's first encrypted TAR header using an account-specific UIN.
///
/// This is only a preliminary format check: CBC does not authenticate the archive.
/// The caller must validate the remaining stream and every archive entry.
///
/// # Errors
/// Returns an error for a malformed UIN, truncated input, or an invalid header.
pub fn verify_v2_header(ciphertext: &[u8], uin: &str) -> Result<()> {
    if uin.len() != 32 || !uin.is_ascii() {
        bail!("Giá trị tài khoản không hợp lệ");
    }
    if ciphertext.len() < 512 {
        bail!("Tệp sao lưu quá ngắn hoặc bị hỏng");
    }
    let key = Sha256::digest(uin.as_bytes());
    let mut iv = [0u8; 16];
    iv[..3].copy_from_slice(b"zie");
    iv[3..].copy_from_slice(&uin.as_bytes()[..13]);
    let mut header = [0u8; 512];
    header.copy_from_slice(&ciphertext[..512]);
    cbc::Decryptor::<Aes256>::new_from_slices(&key, &iv)
        .map_err(|_| anyhow::anyhow!("Không thể khởi tạo giải mã"))?
        .decrypt_padded_mut::<NoPadding>(&mut header)
        .map_err(|_| anyhow::anyhow!("Không thể đọc bản sao lưu"))?;

    if &header[257..262] != b"ustar" {
        bail!("Giá trị tài khoản không đúng hoặc bản sao lưu không được hỗ trợ");
    }
    let checksum = std::str::from_utf8(&header[148..156])?.trim_matches(|c| c == '\0' || c == ' ');
    let expected = u64::from_str_radix(checksum, 8)
        .map_err(|_| anyhow::anyhow!("Phần đầu bản sao lưu bị hỏng"))?;
    let actual: u64 = header
        .iter()
        .enumerate()
        .map(|(i, &b)| {
            if (148..156).contains(&i) {
                u64::from(b' ')
            } else {
                u64::from(b)
            }
        })
        .sum();
    if expected != actual {
        bail!("Phần đầu bản sao lưu bị hỏng");
    }
    Ok(())
}

/// Check archive entries before extraction; this function does not write files.
///
/// # Errors
/// Returns an error for invalid or unsafe entries, duplicate destinations, resource
/// limits, or incomplete file bodies. Successful validation is not authentication.
pub fn validate_tar<R: Read>(input: R) -> Result<()> {
    const MAX_ENTRIES: usize = 100_000;
    const MAX_TOTAL_BYTES: u64 = 20 * 1024 * 1024 * 1024;
    const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
    let mut archive = tar::Archive::new(input);
    let mut destinations = HashSet::new();
    let mut regular_files = HashSet::new();
    let mut ancestor_paths = HashSet::new();
    let mut total = 0u64;
    let mut count = 0usize;
    // Raw entries prevent the TAR parser from buffering unbounded PAX or GNU metadata.
    for item in archive.entries()?.raw(true) {
        let mut entry = item?;
        count += 1;
        if count > MAX_ENTRIES {
            bail!("Bản sao lưu có quá nhiều tệp");
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            bail!("Bản sao lưu chứa mục không được hỗ trợ");
        }
        let bytes = entry.path_bytes();
        let name = std::str::from_utf8(&bytes)
            .map_err(|_| anyhow::anyhow!("Đường dẫn trong bản sao lưu không hợp lệ"))?;
        let name = if kind.is_dir() {
            name.trim_end_matches('/')
        } else {
            name
        };
        if name.is_empty() || name.starts_with('/') || name.contains(['\\', ':']) {
            bail!("Đường dẫn trong bản sao lưu không an toàn");
        }
        let mut parts = Vec::new();
        for part in name.split('/') {
            let upper = part.split('.').next().unwrap_or("").to_ascii_uppercase();
            let reserved = matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || (upper.len() == 4
                    && (upper.starts_with("COM") || upper.starts_with("LPT"))
                    && matches!(upper.as_bytes()[3], b'1'..=b'9'));
            if part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with(['.', ' '])
                || part.contains(['<', '>', '"', '|', '?', '*'])
                || part.chars().any(char::is_control)
                || reserved
            {
                bail!("Đường dẫn trong bản sao lưu không an toàn");
            }
            parts.push(part.to_lowercase());
        }
        let dest = parts.join("/");
        for i in 1..parts.len() {
            let ancestor = parts[..i].join("/");
            if regular_files.contains(&ancestor) {
                bail!("Đường dẫn trong bản sao lưu bị xung đột");
            }
            ancestor_paths.insert(ancestor);
        }
        if !destinations.insert(dest.clone()) {
            bail!("Bản sao lưu có đường dẫn trùng lặp");
        }
        if kind.is_file() {
            if ancestor_paths.contains(&dest) {
                bail!("Đường dẫn trong bản sao lưu bị xung đột");
            }
            regular_files.insert(dest);
        }
        let size = entry.size();
        total = total
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("Bản sao lưu quá lớn"))?;
        if size > MAX_FILE_BYTES || total > MAX_TOTAL_BYTES {
            bail!("Bản sao lưu quá lớn");
        }
        if std::io::copy(&mut entry, &mut std::io::sink())? != size {
            bail!("Tệp sao lưu bị cắt ngắn");
        }
    }
    if count == 0 {
        bail!("Bản sao lưu không có dữ liệu");
    }
    // The TAR iterator accepts bare EOF; require the second end marker and
    // consume the rest so a truncated or appended archive cannot pass silently.
    let mut input = archive.into_inner();
    let mut end = [0u8; 512];
    input
        .read_exact(&mut end)
        .map_err(|_| anyhow::anyhow!("Tệp sao lưu bị cắt ngắn"))?;
    if end.iter().any(|b| *b != 0) {
        bail!("Phần cuối bản sao lưu không hợp lệ");
    }
    let mut trailing = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        if buffer[..n].iter().any(|b| *b != 0) {
            bail!("Phần cuối bản sao lưu không hợp lệ");
        }
        trailing += n as u64;
    }
    if !trailing.is_multiple_of(512) {
        bail!("Phần cuối bản sao lưu không hợp lệ");
    }
    Ok(())
}

/// Seekable CBC reader over a validated V2 stream.
pub struct EncryptedTarReader<'a, R> {
    source: R,
    key: [u8; 32],
    iv: [u8; 16],
    pos: u64,
    len: u64,
    cancel: &'a std::sync::atomic::AtomicBool,
}

impl<R> Drop for EncryptedTarReader<'_, R> {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key.zeroize();
        self.iv.zeroize();
    }
}

impl<'a, R: Read + std::io::Seek> EncryptedTarReader<'a, R> {
    /// Open and verify a Desktop V2 stream.
    pub fn new(
        mut source: R,
        uin: &str,
        cancel: &'a std::sync::atomic::AtomicBool,
    ) -> Result<Self> {
        use std::io::SeekFrom;
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            bail!("Đã dừng nhập bản sao lưu");
        }
        let size = source.seek(SeekFrom::End(0))?;
        if size < 528 || size % 16 != 0 {
            bail!("Tệp sao lưu bị cắt ngắn");
        }
        source.seek(SeekFrom::Start(0))?;
        let mut header = [0u8; 512];
        source.read_exact(&mut header)?;
        verify_v2_header(&header, uin)?;
        let mut key = [0u8; 32];
        key.copy_from_slice(&Sha256::digest(uin.as_bytes()));
        let mut iv = [0u8; 16];
        iv[..3].copy_from_slice(b"zie");
        iv[3..].copy_from_slice(&uin.as_bytes()[..13]);
        source.seek(SeekFrom::Start(size - 32))?;
        let mut prev = [0u8; 16];
        let mut last = [0u8; 16];
        source.read_exact(&mut prev)?;
        source.read_exact(&mut last)?;
        let tail_len = cbc::Decryptor::<Aes256>::new_from_slices(&key, &prev)
            .map_err(|_| anyhow::anyhow!("Không thể khởi tạo giải mã"))?
            .decrypt_padded_mut::<Pkcs7>(&mut last)
            .map_err(|_| anyhow::anyhow!("Tệp sao lưu bị hỏng"))?
            .len();
        Ok(Self {
            source,
            key,
            iv,
            pos: 0,
            len: size - 16 + tail_len as u64,
            cancel,
        })
    }
}

impl<R: Read + std::io::Seek> Read for EncryptedTarReader<'_, R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        use std::io::{Error, ErrorKind, SeekFrom};
        if self.cancel.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(Error::other("cancelled"));
        }
        if self.pos >= self.len || out.is_empty() {
            return Ok(0);
        }
        let count = out.len().min(64 * 1024).min((self.len - self.pos) as usize);
        let start = self.pos / 16 * 16;
        let offset = (self.pos - start) as usize;
        let bytes = (offset + count).div_ceil(16) * 16;
        let mut previous = self.iv;
        if start > 0 {
            self.source.seek(SeekFrom::Start(start - 16))?;
            self.source.read_exact(&mut previous)?;
        }
        self.source.seek(SeekFrom::Start(start))?;
        let mut buffer = vec![0u8; bytes];
        self.source.read_exact(&mut buffer)?;
        cbc::Decryptor::<Aes256>::new_from_slices(&self.key, &previous)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "cipher"))?
            .decrypt_padded_mut::<NoPadding>(&mut buffer)
            .map_err(|_| Error::new(ErrorKind::InvalidData, "cipher"))?;
        out[..count].copy_from_slice(&buffer[offset..offset + count]);
        use zeroize::Zeroize;
        buffer.zeroize();
        self.pos += count as u64;
        Ok(count)
    }
}

impl<R: Read + std::io::Seek> std::io::Seek for EncryptedTarReader<'_, R> {
    fn seek(&mut self, from: std::io::SeekFrom) -> std::io::Result<u64> {
        use std::io::{Error, ErrorKind, SeekFrom};
        let next = match from {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::End(n) => self.len as i128 + n as i128,
            SeekFrom::Current(n) => self.pos as i128 + n as i128,
        };
        if next < 0 || next > u64::MAX as i128 {
            return Err(Error::new(ErrorKind::InvalidInput, "seek"));
        }
        self.pos = next as u64;
        Ok(self.pos)
    }
}
