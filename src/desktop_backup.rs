//! Desktop V2 dry-run and transactional import through the fork's replay-safe store.
use crate::desktop_backup_format::{validate_tar, EncryptedTarReader};
use crate::{
    session::Session,
    store::{MessageStore, StoredMessage, StoredThread},
};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{BufRead, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

/// A dry-run fingerprint and counts; no secret key or message content is retained here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub fingerprint: String,
    pub threads: usize,
    pub records: usize,
    pub unsupported: usize,
    pub media: usize,
    pub imported: usize,
    pub skipped: usize,
}
struct Asset {
    offset: u64,
    size: u64,
    name: String,
}
fn check(session: &Session, cancel: &AtomicBool) -> Result<()> {
    ensure!(
        session.is_active() && !cancel.load(Ordering::Acquire),
        "Đã dừng nhập backup"
    );
    Ok(())
}
fn identity(value: &Value, key: &str) -> Result<String> {
    let raw = value.get(key).context("Thiếu định danh backup")?;
    let value = raw
        .as_str()
        .map(str::to_owned)
        .or_else(|| raw.as_u64().map(|v| v.to_string()))
        .context("Định danh backup không hợp lệ")?;
    ensure!(
        !value.is_empty() && value.len() <= 128,
        "Định danh backup không hợp lệ"
    );
    Ok(value)
}
fn line(reader: &mut impl BufRead) -> Result<Option<Value>> {
    loop {
        let mut bytes = Vec::new();
        let n = reader
            .by_ref()
            .take(4 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut bytes)?;
        if n == 0 {
            return Ok(None);
        }
        ensure!(n <= 4 * 1024 * 1024, "Bản ghi backup quá lớn");
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        return Ok(Some(
            serde_json::from_slice(&bytes).context("Bản ghi backup sai định dạng")?,
        ));
    }
}
fn fingerprint(path: &Path, session: &Session, cancel: &AtomicBool) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.is_file() && file.metadata()?.len() <= 20 * 1024 * 1024 * 1024,
        "Backup quá lớn hoặc không phải tệp"
    );
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        check(session, cancel)?;
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn record(
    value: &Value,
    session: &Session,
    threads: &HashMap<String, StoredThread>,
) -> Result<StoredMessage> {
    let id = identity(value, "msgId")?;
    let mut thread = identity(value, "toUid")?;
    if thread == "0" {
        thread = session.account_key().into();
    }
    ensure!(
        threads.contains_key(&thread),
        "Tin không khớp danh sách hội thoại backup"
    );
    let mut sender = identity(value, "fromUid")?;
    if sender == "0" {
        sender = session.account_key().into();
    }
    let ts = value
        .get("serverTime")
        .or_else(|| value.get("sendDttm"))
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
        .context("Thiếu thời gian tin backup")?;
    ensure!(ts > 0, "Thời gian tin backup không hợp lệ");
    let kind = match value.get("msgType").and_then(Value::as_i64) {
        Some(1) => "chat.text",
        Some(2 | 7) => "chat.photo",
        Some(4) => "chat.sticker",
        Some(18) => "chat.video",
        Some(19) => "chat.file",
        _ => "backup.unknown",
    };
    let mut normalized = value.clone();
    normalized["msgId"] = json!(id);
    normalized["msgType"] = json!(kind);
    normalized["uidFrom"] = json!(sender);
    normalized["ts"] = json!(ts);
    normalized["content"] = value
        .get("message")
        .cloned()
        .unwrap_or(json!("[Tin nhắn chưa được hỗ trợ]"));
    if kind == "backup.unknown" {
        normalized["content"] = json!("[Tin nhắn chưa được hỗ trợ]");
    }
    if let Some(object) = normalized.get_mut("content").and_then(Value::as_object_mut) {
        if !object.contains_key("href") {
            if let Some(url) = object.get("oriUrl").cloned() {
                object.insert("href".into(), url);
            }
        }
        if !object.contains_key("thumb") {
            if let Some(url) = object.get("thumbUrl").cloned() {
                object.insert("thumb".into(), url);
            }
        }
    }
    let mut parsed =
        crate::chat::parse_chat_message(&normalized).context("Không đọc được tin backup")?;
    parsed.is_outgoing = sender == session.account_key();
    parsed.history_source = Some(json!({"desktop_backup":value}));
    let mut stored = StoredMessage::from(&parsed);
    stored.msg_id = id;
    stored.thread_id = thread;
    stored.sender_id = sender;
    stored.msg_type = kind.into();
    stored.ts = ts;
    Ok(stored)
}
fn asset_prefix(name: &str) -> Option<String> {
    let mut parts = name.splitn(4, '_');
    let client = parts.next()?;
    let sender = parts.next()?;
    let target = parts.next()?.split('.').next()?;
    let target = target
        .strip_prefix('g')
        .or_else(|| target.strip_prefix('G'))
        .unwrap_or(target);
    if [client, sender, target]
        .iter()
        .any(|id| id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    Some(format!("{client}_{sender}_{target}"))
}
fn record_prefix(record: &Value, account: &str) -> Option<String> {
    let client = identity(record, "cliMsgId").ok()?;
    let sender = identity(record, "fromUid").ok()?;
    let target = identity(record, "toUid").ok()?;
    asset_prefix(&format!(
        "{client}_{}_{}.",
        if sender == "0" { account } else { &sender },
        if target == "0" { account } else { &target }
    ))
}
fn asset_matches(value: &Value, account: &str, name: &str) -> bool {
    let (Ok(client), Ok(mut sender), Ok(mut target)) = (
        identity(value, "cliMsgId"),
        identity(value, "fromUid"),
        identity(value, "toUid"),
    ) else {
        return false;
    };
    if sender == "0" {
        sender = account.into();
    }
    if target == "0" {
        target = account.into();
    }
    let target = target
        .strip_prefix('g')
        .or_else(|| target.strip_prefix('G'))
        .unwrap_or(&target);
    if !client.bytes().all(|b| b.is_ascii_digit())
        || !sender.bytes().all(|b| b.is_ascii_digit())
        || !target.bytes().all(|b| b.is_ascii_digit())
    {
        return false;
    }
    for variant in [
        target.to_owned(),
        format!("g{target}"),
        format!("G{target}"),
    ] {
        if matches!(value.get("msgType").and_then(Value::as_i64), Some(18 | 19))
            && name.starts_with(&format!("{client}_{sender}_{variant}."))
        {
            return true;
        }
        for field in ["oriUrl", "hdUrl", "normalUrl", "thumbUrl", "href"] {
            if let Some(url) = value
                .get("message")
                .and_then(|message| message.get(field))
                .and_then(Value::as_str)
            {
                if name.starts_with(&format!(
                    "{client}_{sender}_{variant}_{}.",
                    crate::crypto::md5_hex(url)
                )) {
                    return true;
                }
            }
        }
    }
    false
}
/// Resolve only generated content-addressed filenames inside this account's durable vault.
pub fn restored_path(
    session: &Session,
    message: &crate::chat::ChatMessage,
    preview: bool,
) -> Option<PathBuf> {
    let data = message.history_source.as_ref()?.get("backup_assets")?;
    let name = data
        .get(if preview { "preview" } else { "original" })
        .or_else(|| data.get("original"))?
        .as_str()?;
    if name.len() != 64 || !name.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(
        session
            .history_path()
            .parent()?
            .join("restored-media")
            .join(name),
    )
}
fn copy_asset(
    reader: &mut impl Read,
    asset: &Asset,
    root: &Path,
    session: &Session,
    cancel: &AtomicBool,
    new_files: &mut Vec<PathBuf>,
) -> Result<String> {
    ensure!(asset.size <= 512 * 1024 * 1024, "Tệp backup quá lớn");
    let mut temp = tempfile::NamedTempFile::new_in(root)?;
    let mut hash = Sha256::new();
    let mut remain = asset.size;
    let mut buffer = [0; 65536];
    while remain > 0 {
        check(session, cancel)?;
        let max = buffer.len().min(remain as usize);
        let n = reader.read(&mut buffer[..max])?;
        ensure!(n > 0, "Tệp backup bị cắt ngắn");
        temp.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
        remain -= n as u64;
    }
    let filename = format!("{:x}", hash.finalize());
    let path = root.join(&filename);
    if !path.exists() {
        temp.persist_noclobber(&path)
            .map_err(|_| anyhow::anyhow!("Không lưu được tệp backup"))?;
        new_files.push(path);
    }
    Ok(filename)
}
/// Validate the entire encrypted archive, then dry-run or import the exact scanned fingerprint.
/// # Errors
/// Validation happens before import. Cancellation rolls back the current short batch; committed batches remain.
pub fn execute(
    path: &Path,
    key: &str,
    session: &Session,
    cancel: Arc<AtomicBool>,
    expected: Option<&Report>,
) -> Result<Report> {
    check(session, &cancel)?;
    let hash = fingerprint(path, session, &cancel)?;
    if let Some(expected) = expected {
        ensure!(
            expected.fingerprint == hash,
            "Backup đã thay đổi; hãy kiểm tra lại trước khi nhập"
        );
    }
    let mut reader = EncryptedTarReader::new(std::fs::File::open(path)?, key, &cancel)?;
    validate_tar(&mut reader)?;
    reader.rewind()?;
    let mut threads = HashMap::new();
    let mut assets: HashMap<String, Vec<Asset>> = HashMap::new();
    let mut asset_count = 0usize;
    let mut messages = 0;
    for entry in tar::Archive::new(&mut reader).entries()?.raw(true) {
        check(session, &cancel)?;
        let entry = entry?;
        let name = String::from_utf8(entry.path_bytes().to_vec())?;
        ensure!(
            name.split('/').next() == Some(session.account_key()),
            "Backup không thuộc tài khoản hiện tại"
        );
        if !entry.header().entry_type().is_file() {
            continue;
        }
        if name.to_ascii_lowercase().ends_with("_zmessage.zdb") {
            messages += 1;
            continue;
        }
        if name.to_ascii_lowercase().ends_with(".zdb") {
            let mut lines = std::io::BufReader::new(entry);
            while let Some(value) = line(&mut lines)? {
                let Ok(id) = identity(&value, "userId") else {
                    continue;
                };
                let group = value
                    .get("isGroup")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                    || value.get("isGroup").and_then(Value::as_i64) == Some(1);
                let thread = StoredThread {
                    thread_id: id.clone(),
                    name: value
                        .get("displayName")
                        .or_else(|| value.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into(),
                    is_group: group,
                    last_ts: 0,
                };
                ensure!(
                    threads.len() < 100000 && threads.insert(id, thread).is_none(),
                    "Danh sách hội thoại backup không hợp lệ"
                );
            }
        } else {
            ensure!(asset_count < 100000, "Quá nhiều tệp backup");
            let filename = name.rsplit('/').next().unwrap_or("");
            let Some(prefix) = asset_prefix(filename) else {
                continue;
            };
            let bucket = assets.entry(prefix).or_default();
            ensure!(bucket.len() < 128, "Quá nhiều biến thể của một tệp backup");
            asset_count += 1;
            bucket.push(Asset {
                offset: entry.raw_file_position(),
                size: entry.size(),
                name: name.rsplit('/').next().unwrap_or("").into(),
            });
        }
    }
    ensure!(
        messages == 1 && !threads.is_empty(),
        "Backup thiếu dữ liệu hội thoại hoặc tin nhắn được hỗ trợ"
    );
    let mut report = Report {
        fingerprint: hash,
        threads: threads.len(),
        media: asset_count,
        ..Default::default()
    };
    let importing = expected.is_some();
    let store = MessageStore::open(session.history_path())?;
    let mut transaction = if importing {
        Some(store.conn.unchecked_transaction()?)
    } else {
        None
    };
    let vault = session
        .history_path()
        .parent()
        .context("Thiếu thư mục tài khoản")?
        .join("restored-media");
    if importing {
        crate::storage::private_dir(&vault)?;
    }
    let mut new_files = Vec::new();
    let result = (|| -> Result<()> {
        if importing {
            for thread in threads.values() {
                store.upsert_thread(thread)?;
            }
        }
        reader.rewind()?;
        let mut asset_reader = EncryptedTarReader::new(std::fs::File::open(path)?, key, &cancel)?;
        for entry in tar::Archive::new(&mut reader).entries()?.raw(true) {
            let entry = entry?;
            if !String::from_utf8_lossy(&entry.path_bytes())
                .to_ascii_lowercase()
                .ends_with("_zmessage.zdb")
            {
                continue;
            }
            let mut lines = std::io::BufReader::new(entry);
            while let Some(value) = line(&mut lines)? {
                check(session, &cancel)?;
                let mut message = record(&value, session, &threads)?;
                report.records += 1;
                if message.msg_type == "backup.unknown" {
                    report.unsupported += 1;
                }
                if importing && report.records.is_multiple_of(250) {
                    if let Some(tx) = transaction.take() {
                        tx.commit()?;
                    }
                    new_files.clear();
                    transaction = Some(store.conn.unchecked_transaction()?);
                }
                if importing {
                    // Do not overwrite a live row with a less expressive backup projection.
                    let exists: bool = store.conn.query_row(
                        "SELECT EXISTS(SELECT 1 FROM messages WHERE thread_id=?1 AND msg_id=?2)",
                        rusqlite::params![message.thread_id, message.msg_id],
                        |row| row.get(0),
                    )?;
                    if exists || store.is_deleted_message(&message)? {
                        report.skipped += 1;
                        continue;
                    }
                    let mut restored = json!({});
                    let prefix = record_prefix(&value, session.account_key());
                    for asset in prefix
                        .as_ref()
                        .and_then(|prefix| assets.get(prefix))
                        .into_iter()
                        .flatten()
                        .filter(|asset| asset_matches(&value, session.account_key(), &asset.name))
                    {
                        asset_reader.seek(SeekFrom::Start(asset.offset))?;
                        let filename = copy_asset(
                            &mut asset_reader,
                            asset,
                            &vault,
                            session,
                            &cancel,
                            &mut new_files,
                        )?;
                        let thumb = value
                            .pointer("/message/thumbUrl")
                            .and_then(Value::as_str)
                            .is_some_and(|url| asset.name.contains(&crate::crypto::md5_hex(url)));
                        if thumb {
                            restored["preview"] = json!(filename);
                        } else {
                            restored["original"] = json!(filename);
                        }
                    }
                    let references = restored.clone();
                    if restored.as_object().is_some_and(|map| !map.is_empty()) {
                        let mut raw: Value = serde_json::from_str(&message.raw_json)?;
                        raw["message_state"]["history_source"]["backup_assets"] = restored;
                        message.raw_json = raw.to_string();
                    }
                    if store.upsert_message(&message)? {
                        report.imported += 1;
                        if let Some(files) = references.as_object() {
                            for (quality, name) in files {
                                if let Some(name) = name.as_str() {
                                    store.conn.execute("INSERT OR REPLACE INTO restored_assets(thread_id,msg_id,quality,name) VALUES(?1,?2,?3,?4)",rusqlite::params![message.thread_id,message.msg_id,quality,name])?;
                                }
                            }
                        }
                    } else {
                        report.skipped += 1;
                        if let Some(files) = references.as_object() {
                            for name in files.values().filter_map(Value::as_str) {
                                store.conn.execute(
                                    "INSERT OR IGNORE INTO restored_gc(name) VALUES(?1)",
                                    [name],
                                )?;
                            }
                        }
                    }
                }
            }
        }
        check(session, &cancel)?;
        if let Some(tx) = transaction.take() {
            tx.commit()?;
        }
        Ok(())
    })();
    if result.is_err() {
        // Committed batches remain; only the current batch and its new media are rolled back.
        drop(transaction);
        for path in new_files {
            let _ = std::fs::remove_file(path);
        }
    }
    result?;
    Ok(report)
}

/// Decode a restored preview with explicit JXL allocation and pixel limits.
/// Format handling is adapted from upstream PR #96 (MIT/Apache-2.0).
pub(crate) fn preview_image(bytes: &[u8], key: &str) -> Result<image::RgbaImage> {
    let jxl = bytes.starts_with(&[0xff, 0x0a])
        || bytes.starts_with(&[0, 0, 0, 12, b'J', b'X', b'L', b' ', 13, 10, 135, 10]);
    if !jxl {
        return crate::limits::preview_image(bytes, key);
    }
    ensure!(bytes.len() <= 64 * 1024 * 1024, "Ảnh backup quá lớn");
    let decoder = jxl_oxide::JxlImage::builder()
        .alloc_tracker(jxl_oxide::AllocTracker::with_limit(128 * 1024 * 1024))
        .read(std::io::Cursor::new(bytes))
        .map_err(|_| anyhow::anyhow!("Không đọc được ảnh JPEG XL"))?;
    ensure!(
        decoder.width() > 0
            && decoder.height() > 0
            && u64::from(decoder.width()) * u64::from(decoder.height()) <= 16_000_000,
        "Kích thước ảnh backup không được hỗ trợ"
    );
    let render = decoder
        .render_frame(0)
        .map_err(|_| anyhow::anyhow!("Không giải mã được ảnh JPEG XL"))?;
    let mut stream = render.stream();
    let (width, height, channels) = (stream.width(), stream.height(), stream.channels() as usize);
    ensure!(
        (1..=4).contains(&channels) && u64::from(width) * u64::from(height) <= 16_000_000,
        "Định dạng ảnh backup không được hỗ trợ"
    );
    let mut pixels = vec![0u8; width as usize * height as usize * channels];
    ensure!(
        stream.write_to_buffer(&mut pixels) == pixels.len(),
        "Ảnh backup chưa đầy đủ"
    );
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for pixel in pixels.chunks_exact(channels) {
        match channels {
            1 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], 255]),
            2 => rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]),
            3 => rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]),
            4 => rgba.extend_from_slice(pixel),
            _ => unreachable!(),
        }
    }
    let image =
        image::RgbaImage::from_raw(width, height, rgba).context("Ảnh backup không hợp lệ")?;
    let limit = if key.starts_with("full_") { 2048 } else { 512 };
    if width <= limit && height <= limit {
        return Ok(image);
    }
    Ok(image::DynamicImage::ImageRgba8(image)
        .thumbnail(limit, limit)
        .to_rgba8())
}

#[cfg(test)]
mod image_tests {
    use super::*;
    #[test]
    fn restores_jxl_without_a_remote_url() {
        let bytes = include_bytes!("../tests/fixtures/3x3_srgb_lossy.jxl");
        assert_eq!(
            preview_image(bytes, "full_backup").unwrap().dimensions(),
            (3, 3)
        );
    }
    #[test]
    fn rejects_truncated_jxl_and_unrecognized_bytes() {
        assert!(preview_image(&[0xff, 0x0a], "full_backup").is_err());
        assert!(preview_image(b"not an image", "full_backup").is_err());
    }
}
