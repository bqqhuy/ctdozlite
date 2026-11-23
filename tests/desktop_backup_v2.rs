//! Synthetic fixture produced with OpenSSL AES-256-CBC, fake account 111 and key `f` × 32.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use zalo_rust::{
    desktop_backup,
    session::{LoginInfo, Session},
    store::MessageStore,
};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn dry_run_import_replay_delete_and_account_binding() {
    let _lock = ENV_LOCK.lock().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::env::set_var("CTDOZ_DATA_DIR", root.path());
    let path = root.path().join("fake.crypt");
    std::fs::write(&path, include_bytes!("fixtures/desktop-v2-fake-111.crypt")).unwrap();
    let mut session = Session::new(false, false, false).unwrap();
    session.login = Some(LoginInfo {
        uin: "111".into(),
        ..Default::default()
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let key = "f".repeat(32);
    let scan = desktop_backup::execute(&path, &key, &session, cancel.clone(), None).unwrap();
    assert_eq!((scan.threads, scan.records, scan.imported), (1, 2, 0));
    let report =
        desktop_backup::execute(&path, &key, &session, cancel.clone(), Some(&scan)).unwrap();
    assert_eq!(report.imported, 2);
    assert_eq!(
        desktop_backup::execute(&path, &key, &session, cancel.clone(), Some(&scan))
            .unwrap()
            .imported,
        0
    );
    let store = MessageStore::open(session.history_path()).unwrap();
    let text: String = store
        .conn
        .query_row(
            "SELECT text FROM messages WHERE thread_id='222' AND msg_id='101'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(text, "Fixture text");
    store
        .conn
        .execute(
            "INSERT INTO deleted_messages(thread_id,msg_id) VALUES('222','101')",
            [],
        )
        .unwrap();
    store
        .conn
        .execute(
            "DELETE FROM messages WHERE thread_id='222' AND msg_id='101'",
            [],
        )
        .unwrap();
    assert_eq!(
        desktop_backup::execute(&path, &key, &session, cancel.clone(), Some(&scan))
            .unwrap()
            .imported,
        0
    );
    let exists: bool = store
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE thread_id='222' AND msg_id='101')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!exists); // (INV-4): a backup cannot resurrect a delete-for-me row.
    failed_batch_preserves_live_wal_and_committed_media(&session, root.path());
    session.login.as_mut().unwrap().uin = "333".into();
    assert!(desktop_backup::execute(&path, &key, &session, cancel, None).is_err());
}

fn failed_batch_preserves_live_wal_and_committed_media(session: &Session, root: &std::path::Path) {
    use cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::sync::atomic::Ordering;

    fn append(tar: &mut tar::Builder<Vec<u8>>, name: &str, data: &[u8]) {
        let mut header = tar::Header::new_ustar();
        header.set_size(data.len() as u64);
        header.set_mode(0o600);
        header.set_cksum();
        tar.append_data(&mut header, name, data).unwrap();
    }

    let mut tar = tar::Builder::new(Vec::new());
    append(
        &mut tar,
        "111/threads.zdb",
        b"{\"userId\":\"222\",\"displayName\":\"Fixture\"}\n",
    );
    let mut rows = Vec::new();
    for i in 1..=251 {
        let id = 900000 + i;
        let media = i == 1 || i == 250;
        let message = if media {
            json!({"title":"fixture.bin","href":"https://example.invalid/fixture.bin"})
        } else {
            // Exceed SQLite's default WAL auto-checkpoint threshold in the first batch.
            json!("x".repeat(32768))
        };
        let row = json!({"msgId":id.to_string(),"cliMsgId":id.to_string(),
            "fromUid":"0","toUid":"222","serverTime":1700000000000i64+i,
            "msgType":if media {19} else {1},"message":message});
        rows.extend_from_slice(row.to_string().as_bytes());
        rows.push(b'\n');
    }
    append(&mut tar, "111/all_zmessage.zdb", &rows);
    let committed_asset = b"committed backup asset";
    let pending_asset = b"uncommitted backup asset";
    append(&mut tar, "111/media/900001_111_222.bin", committed_asset);
    append(&mut tar, "111/media/900250_111_222.bin", pending_asset);
    let plain = tar.into_inner().unwrap();
    let key = "f".repeat(32);
    let cipher_key = Sha256::digest(key.as_bytes());
    let mut iv = [0u8; 16];
    iv[..3].copy_from_slice(b"zie");
    iv[3..].copy_from_slice(&key.as_bytes()[..13]);
    let len = plain.len();
    let mut encrypted = plain;
    encrypted.resize(len + 16, 0);
    let ciphertext = cbc::Encryptor::<aes::Aes256>::new_from_slices(&cipher_key, &iv)
        .unwrap()
        .encrypt_padded_mut::<Pkcs7>(&mut encrypted, len)
        .unwrap();
    let path = root.join("batched-fixture.crypt");
    std::fs::write(&path, ciphertext).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let scan = desktop_backup::execute(&path, &key, session, cancel.clone(), None).unwrap();
    assert_eq!(scan.records, 251);

    let store = MessageStore::open(session.history_path()).unwrap();
    store
        .conn
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
    store.conn.execute("INSERT INTO messages(msg_id,thread_id,sender_id,sender_name,text,msg_type,ts,is_outgoing,raw_json) VALUES('live-before-import','222','111','','live','chat.text',1,1,'{}')", []).unwrap();
    store.conn.execute_batch("CREATE TRIGGER reject_fixture_message BEFORE INSERT ON messages WHEN NEW.msg_id='900251' BEGIN SELECT RAISE(ABORT,'synthetic import failure'); END;").unwrap();
    let error =
        desktop_backup::execute(&path, &key, session, cancel.clone(), Some(&scan)).unwrap_err();
    assert!(
        error.to_string().contains("synthetic import failure"),
        "{error:#}"
    );
    let exists = |id: &str| -> bool {
        store
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE thread_id='222' AND msg_id=?1)",
                [id],
                |row| row.get(0),
            )
            .unwrap()
    };
    assert!(exists("live-before-import"));
    assert!(exists("900001"));
    assert!(exists("900249"));
    assert!(!exists("900250"));
    assert!(!exists("900251"));
    let vault = session
        .history_path()
        .parent()
        .unwrap()
        .join("restored-media");
    assert!(vault
        .join(format!("{:x}", Sha256::digest(committed_asset)))
        .exists());
    assert!(!vault
        .join(format!("{:x}", Sha256::digest(pending_asset)))
        .exists());

    // Read a copy of the main file without its WAL: the first commit was checkpointed.
    let main_copy = root.join("checkpointed-main.db");
    std::fs::copy(session.history_path(), &main_copy).unwrap();
    let checkpointed = MessageStore::open(&main_copy).unwrap();
    let checkpointed_live: bool = checkpointed
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE msg_id='live-before-import')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(checkpointed_live);

    cancel.store(true, Ordering::Release);
    assert!(desktop_backup::execute(&path, &key, session, cancel.clone(), Some(&scan)).is_err());
    assert!(exists("live-before-import"));
    cancel.store(false, Ordering::Release);
    store
        .conn
        .execute_batch("DROP TRIGGER reject_fixture_message;")
        .unwrap();
    let replay = desktop_backup::execute(&path, &key, session, cancel, Some(&scan)).unwrap();
    assert_eq!(replay.imported, 2);
    assert!(exists("900250"));
    assert!(exists("900251"));
    assert!(vault
        .join(format!("{:x}", Sha256::digest(pending_asset)))
        .exists());
}

#[test]
fn import_narrow_write_lock_allows_concurrent_realtime_writes() {
    let _lock = ENV_LOCK.lock().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::env::set_var("CTDOZ_DATA_DIR", root.path());
    let path = root.path().join("fake.crypt");
    std::fs::write(&path, include_bytes!("fixtures/desktop-v2-fake-111.crypt")).unwrap();

    let mut session = Session::new(false, false, false).unwrap();
    session.login = Some(LoginInfo {
        uin: "111".into(),
        ..Default::default()
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let key = "f".repeat(32);
    let _ = std::fs::remove_file(session.history_path());
    let _ = std::fs::remove_file(format!("{}-wal", session.history_path().display()));
    let _ = std::fs::remove_file(format!("{}-shm", session.history_path().display()));

    let scan = desktop_backup::execute(&path, &key, &session, cancel.clone(), None).unwrap();

    // Channel-coordinated proof that realtime writer can acquire write permit
    // while backup import is inside the media preparation phase outside permit.
    let (tx_in_media, rx_in_media) = std::sync::mpsc::channel();
    let (tx_allow_resume, rx_allow_resume) = std::sync::mpsc::channel();
    let tx_in_media = std::sync::Arc::new(std::sync::Mutex::new(tx_in_media));
    let rx_allow_resume = std::sync::Arc::new(std::sync::Mutex::new(rx_allow_resume));
    let triggered = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Hook cleanup guard ensuring global hook is reset on test exit or panic
    struct HookGuard;
    impl Drop for HookGuard {
        fn drop(&mut self) {
            *desktop_backup::TEST_IMPORT_MEDIA_HOOK
                .write()
                .unwrap_or_else(|e| e.into_inner()) = None;
        }
    }
    let _guard = HookGuard;

    let tx_clone = tx_in_media.clone();
    let rx_clone = rx_allow_resume.clone();
    let trig_clone = triggered.clone();
    *desktop_backup::TEST_IMPORT_MEDIA_HOOK
        .write()
        .unwrap_or_else(|e| e.into_inner()) = Some(std::sync::Arc::new(move || {
        if !trig_clone.swap(true, std::sync::atomic::Ordering::SeqCst) {
            let _ = tx_clone.lock().unwrap().send(());
            if rx_clone
                .lock()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(5))
                .is_err()
            {
                panic!("import media hook timed out waiting for test runner resume signal");
            }
        }
    }));

    let session_for_import = session.clone();
    let path_clone = path.clone();
    let key_clone = key.clone();
    let cancel_clone = cancel.clone();
    let scan_clone = scan.clone();
    let import_handle = std::thread::spawn(move || {
        desktop_backup::execute(
            &path_clone,
            &key_clone,
            &session_for_import,
            cancel_clone,
            Some(&scan_clone),
        )
    });

    // 1. Wait until import is confirmed to be inside the asset copy loop
    rx_in_media
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("import must enter media preparation phase");

    // 2. Realtime writer runs in a separate thread, acquiring account write permit concurrently
    let (tx_writer, rx_writer) = std::sync::mpsc::channel();
    let session_for_writer = session.clone();
    let writer_handle = std::thread::spawn(move || {
        let res: Result<(), String> = (|| {
            let (store, _permit) = MessageStore::open_for_account_write(&session_for_writer)
                .map_err(|e| e.to_string())?;
            let message = zalo_rust::store::StoredMessage {
                msg_id: "rt-live-1".into(),
                thread_id: "222".into(),
                sender_id: "sender-1".into(),
                sender_name: "Live User".into(),
                text: "Hello from realtime stream".into(),
                msg_type: "chat.text".into(),
                ts: 1_700_000_000,
                is_outgoing: false,
                raw_json: "{}".into(),
            };
            store.upsert_message(&message).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let _ = tx_writer.send(res);
    });

    // Writer must acquire permit and commit concurrently while import is in media phase (within 2s)
    let writer_res = rx_writer
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect(
            "writer must acquire permit and commit concurrently while import is in media phase",
        );
    writer_res.expect("realtime write must succeed");
    writer_handle.join().unwrap();

    // Query DB to assert realtime message exists right now
    {
        let store = MessageStore::open(session.history_path()).unwrap();
        let rt_exists: bool = store
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE thread_id='222' AND msg_id='rt-live-1')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            rt_exists,
            "Realtime message must be committed while import is paused in media phase"
        );
    }

    // 3. Signal import to resume and complete its batch commit
    tx_allow_resume.send(()).unwrap();
    let report = import_handle.join().unwrap().expect("import must succeed");
    assert_eq!(report.imported, 2);

    // Reset hook
    *desktop_backup::TEST_IMPORT_MEDIA_HOOK.write().unwrap() = None;

    // 4. Verify both realtime and imported backup messages coexist in local store
    let store = MessageStore::open(session.history_path()).unwrap();
    let rt_exists: bool = store
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE thread_id='222' AND msg_id='rt-live-1')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        rt_exists,
        "Realtime message must remain intact after import"
    );

    let backup_exists: bool = store
        .conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM messages WHERE thread_id='222' AND msg_id='101')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(backup_exists, "Backup message must be committed by import");
}

#[test]
fn import_cancelled_mid_media_phase_aborts_without_committing_corrupt_data() {
    let _lock = ENV_LOCK.lock().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::env::set_var("CTDOZ_DATA_DIR", root.path());
    let path = root.path().join("fake.crypt");
    std::fs::write(&path, include_bytes!("fixtures/desktop-v2-fake-111.crypt")).unwrap();

    let mut session = Session::new(false, false, false).unwrap();
    session.login = Some(LoginInfo {
        uin: "111".into(),
        ..Default::default()
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let key = "f".repeat(32);
    let _ = std::fs::remove_file(session.history_path());
    let _ = std::fs::remove_file(format!("{}-wal", session.history_path().display()));
    let _ = std::fs::remove_file(format!("{}-shm", session.history_path().display()));

    let scan = desktop_backup::execute(&path, &key, &session, cancel.clone(), None).unwrap();

    let cancel_in_hook = cancel.clone();
    *desktop_backup::TEST_IMPORT_MEDIA_HOOK
        .write()
        .unwrap_or_else(|e| e.into_inner()) = Some(std::sync::Arc::new(move || {
        cancel_in_hook.store(true, Ordering::Release);
    }));

    let err = desktop_backup::execute(&path, &key, &session, cancel, Some(&scan)).unwrap_err();
    assert!(
        err.to_string().contains("Đã dừng nhập backup"),
        "Must abort with cancellation: {err}"
    );

    // Reset hook
    *desktop_backup::TEST_IMPORT_MEDIA_HOOK.write().unwrap() = None;

    // Verify uncommitted backup batch was not partially written
    let store = MessageStore::open(session.history_path()).unwrap();
    let count: i64 = store
        .conn
        .query_row(
            "SELECT count(*) FROM messages WHERE msg_id='101'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 0,
        "No messages should be committed when cancelled mid-phase"
    );
}

#[test]
fn import_fails_cleanly_on_account_switch_or_logout() {
    let _lock = ENV_LOCK.lock().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::env::set_var("CTDOZ_DATA_DIR", root.path());
    let path = root.path().join("fake.crypt");
    std::fs::write(&path, include_bytes!("fixtures/desktop-v2-fake-111.crypt")).unwrap();

    let mut session = Session::new(false, false, false).unwrap();
    session.login = Some(LoginInfo {
        uin: "111".into(),
        ..Default::default()
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let key = "f".repeat(32);
    let _ = std::fs::remove_file(session.history_path());
    let _ = std::fs::remove_file(format!("{}-wal", session.history_path().display()));
    let _ = std::fs::remove_file(format!("{}-shm", session.history_path().display()));

    let scan = desktop_backup::execute(&path, &key, &session, cancel.clone(), None).unwrap();

    // Log out before import executes
    session.logout();
    let err = desktop_backup::execute(&path, &key, &session, cancel, Some(&scan)).unwrap_err();
    assert!(
        err.to_string().contains("không thuộc tài khoản")
            || err.to_string().contains("Phiên đã đóng")
            || err.to_string().contains("chưa đăng nhập"),
        "Must reject writing when account is logged out: {}",
        err
    );
}
