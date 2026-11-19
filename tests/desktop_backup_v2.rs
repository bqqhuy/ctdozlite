//! Synthetic fixture produced with OpenSSL AES-256-CBC, fake account 111 and key `f` × 32.
use std::sync::{atomic::AtomicBool, Arc};
use zalo_rust::{
    desktop_backup,
    session::{LoginInfo, Session},
    store::MessageStore,
};

#[test]
fn dry_run_import_replay_delete_and_account_binding() {
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
