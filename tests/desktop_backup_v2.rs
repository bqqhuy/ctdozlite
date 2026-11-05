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
    session.login.as_mut().unwrap().uin = "333".into();
    assert!(desktop_backup::execute(&path, &key, &session, cancel, None).is_err());
}
