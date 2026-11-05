//! Explicit V1 RSA/ZDB4 fallback, isolated from Sync2 coverage and key material.
use crate::{
    chat::Thread,
    session::Session,
    socket::{SyncTransport, WsEvent},
};
use anyhow::{ensure, Context, Result};
use aws_lc_rs::{
    encoding::AsDer,
    rsa::{KeySize, Pkcs1PrivateDecryptingKey, PrivateDecryptingKey},
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use cipher::{BlockDecryptMut, KeyIvInit};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use zeroize::Zeroizing;

/// Unwrap the phone's PKCS#1-v1.5 secret using the AWS-LC backend.
fn unwrap_key(key: PrivateDecryptingKey, wrapped: &str) -> Result<Zeroizing<Vec<u8>>> {
    ensure!(wrapped.len() <= 4096, "Khóa gói V1 không hợp lệ");
    let ciphertext = B64.decode(wrapped).context("Khóa gói V1 sai định dạng")?;
    let key = Pkcs1PrivateDecryptingKey::new(key)
        .map_err(|_| anyhow::anyhow!("Không khởi tạo được khóa V1"))?;
    let mut secret = Zeroizing::new(vec![0u8; key.min_output_size()]);
    let len = key
        .decrypt(&ciphertext, &mut secret)
        .map_err(|_| anyhow::anyhow!("Gói V1 không thuộc khóa của phiên này"))?
        .len();
    secret.truncate(len);
    Ok(secret)
}

/// The PC-compatible AES key is uppercase hex of the first 16 unwrapped bytes.
fn derive_aes_key(secret: &[u8]) -> Result<Zeroizing<String>> {
    ensure!(secret.len() >= 16, "Khóa AES của gói V1 quá ngắn");
    Ok(Zeroizing::new(
        secret[..16]
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect(),
    ))
}
/// Each 64 KiB segment restarts CBC with a zero IV; any unaligned tail is copied by the caller.
fn decrypt_chunk(key: &str, chunk: &mut [u8]) -> Result<()> {
    cbc::Decryptor::<aes::Aes256>::new_from_slices(key.as_bytes(), &[0; 16])?
        .decrypt_padded_mut::<cipher::block_padding::NoPadding>(chunk)
        .map_err(|_| anyhow::anyhow!("Không giải mã được gói V1"))?;
    Ok(())
}

const MAX_DECODED: u64 = 2 * 1024 * 1024 * 1024;
/// Stream XZ with a dictionary/decoder budget checked before allocation.
fn decompress_xz(input: impl Read, output: &mut impl Write, memory_kib: u32) -> Result<u64> {
    let mut reader = lzma_rust2::XzReader::new_mem_limit(input, false, memory_kib);
    Ok(std::io::copy(&mut reader, output)?)
}

struct Archive {
    _dir: tempfile::TempDir,
    entries: Vec<Entry>,
}
struct Entry {
    local: String,
    group: bool,
    path: PathBuf,
}
#[derive(Debug, PartialEq)]
enum Control {
    Confirm,
    Idle,
    Active,
    Reject,
    Restoring,
    Ready { url: String, key: String },
    Failed(i64),
}
fn integer(value: Option<&Value>) -> Option<i64> {
    value.and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()))
}
fn controls(event: &WsEvent, public: &str, computer: &str) -> Vec<Control> {
    let data = event.payload.get("data").unwrap_or(&event.payload);
    let list = data
        .get("controls")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    list.iter()
        .filter_map(|item| {
            let content = item.get("content").unwrap_or(item);
            if !matches!(
                content.get("act_type").and_then(Value::as_str),
                Some("syncmsgmb" | "transfer_msg")
            ) {
                return None;
            }
            let data = match content.get("data")? {
                Value::String(text) if text.len() <= 65536 => serde_json::from_str(text).ok()?,
                value if value.is_object() => value.clone(),
                _ => return None,
            };
            if data.get("public_key").and_then(Value::as_str) != Some(public) {
                return None;
            }
            let act = content.get("act")?.as_str()?;
            if act == "syncmsg_info" {
                return Some(Control::Ready {
                    url: data.get("url")?.as_str()?.to_owned(),
                    key: data.get("encrypted_key")?.as_str()?.to_owned(),
                });
            }
            if data.get("pc_name").and_then(Value::as_str) != Some(computer) {
                return None;
            }
            match act {
                "user_confirm" => match integer(data.get("user_action"))? {
                    1 | 3 => Some(Control::Confirm),
                    0 => Some(Control::Reject),
                    2 => Some(Control::Restoring),
                    _ => None,
                },
                "transfer_error" => {
                    if let Some(code) = integer(data.get("error_code")).filter(|code| *code != 0) {
                        return Some(Control::Failed(code));
                    }
                    match integer(data.get("status"))? {
                        1 => Some(Control::Active),
                        2 => Some(Control::Idle),
                        _ => None,
                    }
                }
                _ => None,
            }
        })
        .collect()
}
async fn request(s: &Session, public: &str, cancel: bool) -> Result<()> {
    let body = if cancel {
        json!({"pc_name":s.computer_name,"public_key":public,"imei":s.imei})
    } else {
        json!({"pc_name":s.computer_name,"public_key":public,"from_seq_id":0,"is_retry":0,"min_seq_id":0,"temp_key":"","imei":s.imei})
    };
    let params = s.encoded_params(&body)?;
    let path = if cancel {
        "/api/message/cancel_pull_mobile_msg"
    } else {
        "/api/message/pull_mobile_msg"
    };
    let response = s
        .get(&format!(
            "{}{path}?{}&params={params}",
            s.domains.file,
            s.common_params()
        ))
        .await?;
    let response = s.maybe_decrypt(response, s.session_key_bytes()).await;
    ensure!(
        response.get("error_code").is_some(),
        "Thiếu xác nhận yêu cầu V1"
    );
    crate::board::data(response)?;
    Ok(())
}
fn check(s: &Session, cancel: &AtomicBool) -> Result<()> {
    ensure!(
        s.is_active() && !cancel.load(Ordering::Acquire),
        "Đã dừng đồng bộ V1"
    );
    Ok(())
}
fn read_u32(reader: &mut impl Read) -> Result<u32> {
    let mut b = [0; 4];
    reader.read_exact(&mut b)?;
    Ok(u32::from_be_bytes(b))
}
struct Limited<W> {
    inner: W,
    written: u64,
    cancel: Arc<AtomicBool>,
    session: Session,
}
impl<W: Write> Write for Limited<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::Acquire) || !self.session.is_active() {
            return Err(std::io::Error::other("cancelled"));
        }
        if bytes.len() as u64 > MAX_DECODED.saturating_sub(self.written) {
            return Err(std::io::Error::other("decoded backup exceeds limit"));
        }
        let n = self.inner.write(bytes)?;
        self.written += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}
fn decrypt_archive(
    session: &Session,
    cancel: Arc<AtomicBool>,
    key: PrivateDecryptingKey,
    wrapped: &str,
    encrypted: &std::path::Path,
) -> Result<Archive> {
    check(session, &cancel)?;
    let secret = unwrap_key(key, wrapped)?;
    let key = derive_aes_key(&secret)?;
    let root = session
        .cache_path()
        .parent()
        .context("Thiếu thư mục tài khoản")?
        .to_path_buf();
    crate::storage::private_dir(&root)?;
    let dir = tempfile::Builder::new()
        .prefix("phone-v1-")
        .tempdir_in(root)?;
    let mut input = std::fs::File::open(encrypted)?;
    let mut clear = tempfile::tempfile_in(dir.path())?;
    let mut chunk = Zeroizing::new(vec![0u8; 65536]);
    loop {
        check(session, &cancel)?;
        let mut len = 0;
        while len < chunk.len() {
            let n = input.read(&mut chunk[len..])?;
            if n == 0 {
                break;
            }
            len += n;
        }
        if len == 0 {
            break;
        }
        let aligned = len / 16 * 16;
        decrypt_chunk(&key, &mut chunk[..aligned])?;
        clear.write_all(&chunk[..len])?;
        if len < 65536 {
            break;
        }
    }
    clear.rewind()?;
    let mut header = [0u8; 14];
    clear.read_exact(&mut header)?;
    ensure!(
        &header[..7] == b"ZDB4.0\0",
        "Định dạng V1 không được hỗ trợ"
    );
    let count = read_u32(&mut clear)? as usize;
    ensure!(count > 0 && count <= 20000, "Số CSDL trong V1 không hợp lệ");
    let mut toc = Vec::new();
    let mut total = 0u64;
    let mut seen = HashSet::new();
    for _ in 0..count {
        let len = read_u32(&mut clear)? as usize;
        ensure!(len > 0 && len <= 128, "Tên CSDL V1 không hợp lệ");
        let mut name = vec![0; len];
        clear.read_exact(&mut name)?;
        let name = String::from_utf8(name)?;
        let stem = name.strip_suffix(".db").context("Mục V1 không phải CSDL")?;
        let (local, group) = stem
            .strip_prefix("group_")
            .map(|id| (id, true))
            .unwrap_or((stem, false));
        ensure!(
            !local.is_empty()
                && local.bytes().all(|b| b.is_ascii_digit())
                && seen.insert(name.clone()),
            "Định danh CSDL V1 không hợp lệ"
        );
        let size = read_u32(&mut clear)? as u64;
        total = total.checked_add(size).context("Gói V1 quá lớn")?;
        ensure!(
            size >= 100 && total <= MAX_DECODED,
            "Kích thước CSDL V1 không hợp lệ"
        );
        toc.push((local.to_owned(), group, size));
    }
    let mut decoded = tempfile::tempfile_in(dir.path())?;
    let mut bounded = Limited {
        inner: &mut decoded,
        written: 0,
        cancel: cancel.clone(),
        session: session.clone(),
    };
    decompress_xz(clear, &mut bounded, 128 * 1024).context("Không giải nén được dữ liệu ZDB4")?;
    ensure!(
        bounded.written == total,
        "Dung lượng CSDL V1 không khớp bảng mục lục"
    );
    decoded.rewind()?;
    let mut entries = Vec::new();
    for (index, (local, group, size)) in toc.into_iter().enumerate() {
        check(session, &cancel)?;
        let path = dir.path().join(format!("{index}.db"));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&path)?;
        let bytes = std::io::copy(&mut (&mut decoded).take(size), &mut file)?;
        ensure!(bytes == size, "CSDL V1 bị cắt ngắn");
        entries.push(Entry { local, group, path });
    }
    Ok(Archive { _dir: dir, entries })
}
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
fn open_db(path: &std::path::Path) -> Result<rusqlite::Connection> {
    let db = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    db.execute_batch("PRAGMA query_only=ON; PRAGMA trusted_schema=OFF;")?;
    Ok(db)
}
async fn resolve(
    s: &Session,
    fids: &[String],
    gids: &[String],
) -> Result<HashMap<(bool, String), String>> {
    let mut result = HashMap::new();
    for (group, ids) in [(false, fids), (true, gids)] {
        for batch in ids.chunks(50) {
            let numeric = batch
                .iter()
                .map(|id| id.parse::<u64>())
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let body = if group {
                json!({"gids":numeric})
            } else {
                json!({"fids":numeric})
            };
            let params = s.encoded_params(&body)?;
            let raw = s
                .post_form(
                    &format!(
                        "{}/api/znoise?{}",
                        crate::endpoints::ZNOISE,
                        s.common_params()
                    ),
                    format!("params={params}"),
                )
                .await?;
            let data = crate::board::data(s.maybe_decrypt(raw, s.session_key_bytes()).await)?;
            let values = data
                .get(if group { "gids" } else { "fids" })
                .and_then(Value::as_array)
                .context("Không đọc được ánh xạ V1")?;
            ensure!(values.len() == batch.len(), "Ánh xạ V1 thiếu định danh");
            for (local, uid) in batch.iter().zip(values) {
                let uid = uid
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| uid.as_u64().map(|id| id.to_string()))
                    .context("UID V1 sai định dạng")?;
                if !uid.is_empty() && uid != "0" && uid.bytes().all(|b| b.is_ascii_digit()) {
                    result.insert((group, local.clone()), uid);
                }
            }
        }
    }
    Ok(result)
}
async fn run<F: FnMut(String), G: FnMut(Thread)>(
    s: &Session,
    transport: SyncTransport,
    cancel: Arc<AtomicBool>,
    progress: &mut F,
    page: &mut G,
    private: PrivateDecryptingKey,
    public: &str,
) -> Result<String> {
    let epoch = transport.epoch().context("WebSocket chưa sẵn sàng")?;
    let mut controls_rx = transport.subscribe();
    request(s, public, false).await?;
    progress("V1: chờ xác nhận trên điện thoại…".into());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(900);
    let (url, wrapped) = loop {
        check(s, &cancel)?;
        ensure!(
            transport.epoch() == Some(epoch),
            "Kết nối V1 đã thay đổi; hãy chạy lại sau khi kết nối ổn định"
        );
        ensure!(
            tokio::time::Instant::now() < deadline,
            "V1 hết thời gian chờ điện thoại"
        );
        match tokio::time::timeout(Duration::from_millis(250), controls_rx.recv()).await {
            Ok(Ok(event)) => {
                let mut ready = None;
                for control in controls(&event, public, &s.computer_name) {
                    match control {
                        Control::Ready { url, key } => ready = Some((url, key)),
                        Control::Confirm | Control::Active => {
                            progress("V1: điện thoại đang chuẩn bị bản sao lịch sử…".into())
                        }
                        Control::Idle => progress(
                            "V1: điện thoại tạm nghỉ; mở Zalo và giữ màn hình sáng…".into(),
                        ),
                        Control::Restoring => {
                            anyhow::bail!("Điện thoại đang khôi phục dữ liệu; thử lại sau")
                        }
                        Control::Reject => anyhow::bail!("Điện thoại đã từ chối V1"),
                        Control::Failed(code) => anyhow::bail!("Điện thoại báo lỗi V1 ({code})"),
                    }
                }
                if let Some(ready) = ready {
                    break ready;
                }
            }
            Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {
                anyhow::bail!("Thiếu sự kiện V1; không thể xác nhận hoàn tất")
            }
            Ok(Err(_)) => anyhow::bail!("Kênh V1 đã đóng"),
            Err(_) => {}
        }
    };
    progress("V1: đang tải bản sao lịch sử…".into());
    let root = s
        .cache_path()
        .parent()
        .context("Thiếu cache tài khoản")?
        .to_path_buf();
    crate::storage::private_dir(&root)?;
    let downloaded = tempfile::Builder::new()
        .prefix("v1-download-")
        .tempdir_in(&root)?;
    let encrypted = downloaded.path().join("backup.crypt");
    super::assets::download_backup(&url, &encrypted, s, &cancel).await?;
    progress("V1: giải mã và kiểm tra CSDL…".into());
    let account = s.clone();
    let token = cancel.clone();
    let archive = tokio::task::spawn_blocking(move || {
        decrypt_archive(&account, token, private, &wrapped, &encrypted)
    })
    .await??;
    let account = s.clone();
    let token = cancel.clone();
    let (archive, fids, gids) = tokio::task::spawn_blocking(move || -> Result<_> {
        let mut fids = HashSet::new();
        let mut gids = Vec::new();
        for entry in &archive.entries {
            check(&account, &token)?;
            if entry.group {
                gids.push(entry.local.clone());
            } else {
                fids.insert(entry.local.clone());
            }
            let db = open_db(&entry.path)?;
            let mut stmt =
                db.prepare("SELECT DISTINCT CAST(SenderId AS TEXT) FROM ChatContent LIMIT 100001")?;
            let values = stmt
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            ensure!(values.len() <= 100000, "Quá nhiều sender trong CSDL V1");
            for id in values {
                if id.bytes().all(|b| b.is_ascii_digit()) && !id.is_empty() {
                    fids.insert(id);
                }
            }
        }
        Ok((archive, fids.into_iter().collect::<Vec<_>>(), gids))
    })
    .await??;
    let mapping = resolve(s, &fids, &gids).await?;
    let sender_map = mapping
        .iter()
        .filter(|((group, _), _)| !*group)
        .map(|((_, local), uid)| (local.clone(), uid.clone()))
        .collect();
    let account = s.clone();
    let token = cancel.clone();
    progress("V1: nhập lịch sử theo định danh đã ánh xạ…".into());
    let (inserted,unmapped,previews)=tokio::task::spawn_blocking(move||->Result<_>{
        let mut store=crate::store::MessageStore::open(account.history_path())?;
        let mut added=0;let mut missing=0;let mut previews=HashMap::new();
        for entry in &archive.entries {
            check(&account,&token)?;
            let Some(uid)=mapping.get(&(entry.group,entry.local.clone()))else{missing+=1;continue;};
            let template=Thread{id:uid.clone(),is_group:entry.group,name:if entry.group{format!("Nhóm {uid}")}else{format!("Người dùng {uid}")},..Default::default()};
            let threads=HashMap::from([(entry.local.clone(),template)]);
            let db=open_db(&entry.path)?;
            let mut stmt=db.prepare("SELECT CAST(SenderId AS TEXT),CAST(CliMsgId AS TEXT),CAST(GlbMsgId AS TEXT),MsgType,TimeStamp,MsgContent,BinNet FROM ChatContent ORDER BY TimeStamp ASC")?;
            let mut rows=stmt.query([])?;let mut batch=Vec::new();
            while let Some(row)=rows.next()? {
                check(&account,&token)?;
                let content:Option<String>=row.get(5)?;
                let binary:Option<Vec<u8>>=row.get(6)?;
                ensure!(content.as_ref().is_none_or(|v|v.len()<=2*1024*1024) && binary.as_ref().is_none_or(|v|v.len()<=2*1024*1024),"Bản ghi V1 quá lớn");
                batch.push(json!({"senderId":row.get::<_,String>(0)?,"clientId":row.get::<_,String>(1)?,"globalId":row.get::<_,String>(2)?,"msgType":row.get::<_,i64>(3)?,"timestamp":row.get::<_,i64>(4)?,"content":content.unwrap_or_default(),"meta":{"v1BinNet":binary.map(|b|B64.encode(b))}}));
                if batch.len()==250 {
                    let result=super::import::save(&mut store,&json!({"partitions":[{"id":entry.local,"messages":std::mem::take(&mut batch)}]}),&threads,&sender_map,account.account_key())?;
                    added+=result.inserted;for mut thread in result.previews { if thread.messages.len()>20 {thread.messages.drain(..thread.messages.len()-20);} previews.insert(thread.id.clone(),thread); }
                }
            }
            if !batch.is_empty(){let result=super::import::save(&mut store,&json!({"partitions":[{"id":entry.local,"messages":batch}]}),&threads,&sender_map,account.account_key())?;added+=result.inserted;for mut thread in result.previews { if thread.messages.len()>20 {thread.messages.drain(..thread.messages.len()-20);} previews.insert(thread.id.clone(),thread); }}
        }
        Ok((added,missing,previews))
    }).await??;
    for thread in previews.into_values() {
        page(thread);
    }
    Ok(format!("V1: đã nhập {inserted} tin mới; {unmapped} hội thoại chưa ánh xạ. Không thay checkpoint Sync2."))
}
/// Run only after an explicit user action; never invoked automatically after Sync2 failure.
/// # Errors
/// Reports incomplete transfers and preserves previously committed message pages.
pub(crate) async fn download<F: FnMut(String), G: FnMut(Thread)>(
    s: &Session,
    transport: SyncTransport,
    cancel: Arc<AtomicBool>,
    mut progress: F,
    mut page: G,
) -> Result<String> {
    check(s, &cancel)?;
    progress("V1: đang tạo khóa riêng cho phiên…".into());
    let key = tokio::task::spawn_blocking(|| {
        PrivateDecryptingKey::generate(KeySize::Rsa2048)
            .map_err(|_| anyhow::anyhow!("Không tạo được khóa V1"))
    })
    .await??;
    let der = key
        .public_key()
        .as_der()
        .map_err(|_| anyhow::anyhow!("Không mã hóa được khóa công khai V1"))?;
    let public = B64.encode(der.as_ref());
    let result = run(s, transport, cancel, &mut progress, &mut page, key, &public).await;
    if result.is_err() && s.is_active() {
        // Cleanup is best-effort; it cannot turn an incomplete transfer into success.
        if request(s, &public, true).await.is_err() {
            progress("V1: chưa xác nhận được việc dừng trên điện thoại".into());
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aws_lc_unwrap_and_spki_match_openssl_fixture() {
        let private = PrivateDecryptingKey::from_pkcs8(include_bytes!(
            "../../tests/fixtures/v1-test-only.pk8"
        ))
        .unwrap();
        let public = private.public_key().as_der().unwrap();
        assert_eq!(
            public.as_ref(),
            include_bytes!("../../tests/fixtures/v1-test-only-spki.der")
        );
        let wrapped = B64.encode(include_bytes!(
            "../../tests/fixtures/v1-test-only-wrapped.bin"
        ));
        assert_eq!(
            unwrap_key(private, &wrapped).unwrap().as_slice(),
            b"0123456789ABCDEF"
        );
        let private = PrivateDecryptingKey::generate(KeySize::Rsa2048).unwrap();
        assert!(unwrap_key(private, &wrapped).is_err());
    }
    #[test]
    fn v1_key_and_segment_decryption_match_an_openssl_known_answer() {
        let secret: Vec<u8> = (0..16).collect();
        let key = derive_aes_key(&secret).unwrap();
        assert_eq!(key.as_str(), "000102030405060708090A0B0C0D0E0F");
        for _ in 0..2 {
            let mut block = *include_bytes!("../../tests/fixtures/v1-fake-aes.bin");
            decrypt_chunk(&key, &mut block).unwrap();
            assert_eq!(&block, b"fake-v1-block!!!");
        }
        assert!(derive_aes_key(&[0; 15]).is_err());
    }
    #[test]
    fn xz_stream_respects_the_decoder_memory_budget_and_rejects_truncation() {
        let bytes = include_bytes!("../../tests/fixtures/v1-fake.xz");
        let mut output = Vec::new();
        assert!(decompress_xz(&bytes[..], &mut output, 512).is_err());
        assert!(output.is_empty());
        decompress_xz(&bytes[..], &mut output, 128 * 1024).unwrap();
        assert_eq!(output, b"fake-v1-sqlite-stream\n".repeat(32));
        assert!(decompress_xz(&bytes[..bytes.len() / 2], &mut Vec::new(), 128 * 1024).is_err());
    }
    #[test]
    fn old_public_key_and_other_computer_controls_cannot_finish_current_session() {
        let event = |key: &str, name: &str, act: &str| WsEvent {
            cmd: 603,
            sub_cmd: 1,
            version: 1,
            payload: json!({"controls":[{"content":{"act_type":"syncmsgmb","act":act,"data":{"public_key":key,"pc_name":name,"user_action":1,"url":"https://example.invalid/file","encrypted_key":"AA=="}}}]}),
        };
        assert!(controls(&event("old", "pc", "syncmsg_info"), "current", "pc").is_empty());
        assert!(controls(&event("current", "other", "user_confirm"), "current", "pc").is_empty());
        assert_eq!(
            controls(&event("current", "pc", "user_confirm"), "current", "pc"),
            vec![Control::Confirm]
        );
        assert!(matches!(
            controls(&event("current", "pc", "syncmsg_info"), "current", "pc").as_slice(),
            [Control::Ready { .. }]
        ));
    }
}
