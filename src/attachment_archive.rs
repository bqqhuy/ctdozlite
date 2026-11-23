//! Explicit ZIP packaging for an attachment draft; never changes or sends the source.
use anyhow::{ensure, Context, Result};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

/// Package one unchanged regular file into an account-owned ZIP for user review.
/// # Errors
/// Rejects changed, empty, oversized or unreadable sources and archive write failures.
pub(crate) async fn prepare(
    source: PathBuf,
    expected: Option<crate::outgoing::FileStamp>,
    directory: PathBuf,
) -> Result<PathBuf> {
    let before = crate::outgoing::FileStamp::read(&source).await?;
    ensure!(
        expected.as_ref().is_none_or(|expected| expected == &before),
        "Tệp gốc đã thay đổi; hãy chọn lại trước khi tạo ZIP"
    );
    let check = source.clone();
    let archive = tokio::task::spawn_blocking(move || create(&source, &directory)).await??;
    ensure!(
        crate::outgoing::FileStamp::read(&check).await? == before,
        "Tệp gốc đã thay đổi trong khi tạo ZIP"
    );
    // Archive drafts survive the worker return and use the existing account-temp cleanup.
    let (file, path) = archive.keep().context("Không giữ được tệp ZIP")?;
    drop(file);
    Ok(path)
}

/// Package a restricted executable file into a temporary ZIP level-0 (Stored) archive
/// named `<original_name>.zip` for EEXC upload; caller holds the temp file until upload completes.
pub(crate) fn create_eecx(
    source: &Path,
    directory: &Path,
) -> Result<(tempfile::NamedTempFile, String)> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .context("Tên tệp không hợp lệ")?;
    let archive = create(source, directory)?;
    Ok((archive, format!("{name}.zip")))
}

fn create(source: &Path, directory: &Path) -> Result<tempfile::NamedTempFile> {
    let mut input = std::fs::File::open(source).context("Không mở được tệp gốc")?;
    let metadata = input.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() > 0,
        "Chỉ tạo ZIP từ tệp thường không rỗng"
    );
    // Leave room for the ZIP headers within the normal attachment upload limit.
    ensure!(
        metadata.len() < crate::upload::MAX_UPLOAD_BYTES - 65536,
        "Tệp quá lớn để đóng gói ZIP trong giới hạn gửi"
    );
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .context("Tên tệp không hợp lệ")?;
    ensure!(
        !name.contains(['/', '\\']) && name != "." && name != "..",
        "Tên tệp không hợp lệ"
    );
    std::fs::create_dir_all(directory)?;
    let output = tempfile::Builder::new()
        .prefix(&format!("{name}-"))
        .suffix(".zip")
        .tempfile_in(directory)?;
    let mut archive = zip::ZipWriter::new(output.reopen()?);
    // Stored ZIP preserves the exact payload with bounded memory and no codec dependency.
    archive.start_file(
        name,
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
    )?;
    let copied = std::io::copy(&mut (&mut input).take(metadata.len()), &mut archive)?;
    ensure!(
        copied == metadata.len(),
        "Tệp thay đổi hoặc thiếu dữ liệu khi tạo ZIP"
    );
    let after = input.metadata()?;
    ensure!(
        after.len() == metadata.len() && after.modified().ok() == metadata.modified().ok(),
        "Tệp gốc đã thay đổi trong khi tạo ZIP"
    );
    let finished_file = archive.finish()?;
    finished_file.sync_all()?;
    let final_len = output.as_file().metadata()?.len();
    ensure!(
        final_len <= crate::upload::MAX_UPLOAD_BYTES,
        "Kích thước gói ZIP sau đóng gói vượt giới hạn tải lên"
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn archive_keeps_unicode_name_and_exact_bytes_without_changing_original() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("Bộ cài thử.exe");
        let bytes = [0, 255, 13, 10, 1, 2, 3];
        std::fs::write(&source, bytes).unwrap();
        let stamp = crate::outgoing::FileStamp::read(&source).await.unwrap();
        let output = prepare(source.clone(), Some(stamp), root.path().join("owned"))
            .await
            .unwrap();
        let mut zip = zip::ZipArchive::new(std::fs::File::open(output).unwrap()).unwrap();
        assert_eq!(zip.len(), 1);
        let mut entry = zip.by_index(0).unwrap();
        assert_eq!(entry.name(), "Bộ cài thử.exe");
        let mut restored = Vec::new();
        entry.read_to_end(&mut restored).unwrap();
        assert_eq!(restored, bytes);
        assert_eq!(std::fs::read(source).unwrap(), bytes);
    }
    #[tokio::test]
    async fn changed_source_does_not_create_an_archive() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("setup.msi");
        std::fs::write(&source, b"before").unwrap();
        let stamp = crate::outgoing::FileStamp::read(&source).await.unwrap();
        std::fs::write(&source, b"changed bytes").unwrap();
        let directory = root.path().join("owned");
        assert!(prepare(source, Some(stamp), directory.clone())
            .await
            .is_err());
        assert!(!directory.exists());
    }

    #[test]
    fn create_eecx_produces_level0_stored_zip_with_single_entry_unicode_multidot_and_temp_cleanup()
    {
        let root = tempfile::tempdir().unwrap();
        let temp_dir = root.path().join("temp");
        let source = root.path().join("Báo cáo.tar.gz.exe");
        let payload = b"MZ-eecx-payload-\x00\x01\x02\xff-end";
        std::fs::write(&source, payload).unwrap();

        let temp_zip_path = {
            let (archive, archive_name) = create_eecx(&source, &temp_dir).unwrap();
            assert_eq!(archive_name, "Báo cáo.tar.gz.exe.zip");
            let archive_path = archive.path().to_path_buf();
            assert!(archive_path.starts_with(&temp_dir));

            let mut zip =
                zip::ZipArchive::new(std::fs::File::open(&archive_path).unwrap()).unwrap();
            assert_eq!(zip.len(), 1);
            let mut entry = zip.by_index(0).unwrap();
            assert_eq!(entry.name(), "Báo cáo.tar.gz.exe");
            assert_eq!(entry.compression(), zip::CompressionMethod::Stored);
            let mut unpacked = Vec::new();
            entry.read_to_end(&mut unpacked).unwrap();
            assert_eq!(unpacked, payload);
            archive_path
        };

        // Temp archive cleaned up automatically on drop; original user file stays intact.
        assert!(!temp_zip_path.exists());
        assert_eq!(std::fs::read(&source).unwrap(), payload);
    }
}
