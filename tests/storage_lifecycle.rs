//! One isolated process/data root exercises the public persistence paths without live accounts.
use zalo_rust::{
    chat,
    session::{LoginInfo, Session},
    storage,
};

#[test]
fn remember_logout_and_account_history_are_isolated() {
    let root = std::env::temp_dir().join(format!(
        "ctdoz-lifecycle-{}",
        zalo_rust::param_cipher::random_hex(24)
    ));
    std::env::set_var("CTDOZ_DATA_DIR", &root);
    storage::init(false).unwrap();
    let instance = storage::lock_instance().unwrap();
    assert!(storage::lock_instance().is_err());
    let mut alice = Session::new(false, false, false).unwrap();
    alice.login = Some(LoginInfo {
        uin: "alice-fixture".into(),
        ..Default::default()
    });
    alice.set_zpw_sek("test-token-never-live".into());
    storage::save_settings(&serde_json::json!({"remember_login": false})).unwrap();
    alice.save_to_file("session.json").unwrap();
    assert!(!storage::session_path("session.json").exists());
    assert!(Session::new(false, false, false)
        .unwrap()
        .load_from_file("session.json")
        .is_err());

    storage::save_settings(&serde_json::json!({"remember_login": true})).unwrap();
    alice.save_to_file("session.json").unwrap();
    let mut restored = Session::new(false, false, false).unwrap();
    restored.load_from_file("session.json").unwrap();
    assert_eq!(restored.account_key(), alice.account_key());
    let threads = vec![chat::Thread {
        id: "contact-fixture".into(),
        name: "Alice private thread".into(),
        preview: "private".into(),
        time: String::new(),
        avatar: None,
        is_group: false,
        messages: Vec::new(),
    }];
    chat::save_threads_cache(&alice, &threads);
    assert_eq!(chat::load_threads_cache(&alice).len(), 1);
    let mut bob = Session::new(false, false, false).unwrap();
    bob.login = Some(LoginInfo {
        uin: "bob-fixture".into(),
        ..Default::default()
    });
    assert_ne!(alice.history_path(), bob.history_path());
    assert!(chat::load_threads_cache(&bob).is_empty());

    let mut store = zalo_rust::store::MessageStore::open(alice.history_path()).unwrap();
    zalo_rust::store::persist_threads(&mut store, &threads).unwrap();
    drop(store);
    let mut bob_history = Vec::new();
    zalo_rust::store::load_history_into_threads(bob.history_path(), &mut bob_history, 200);
    assert!(bob_history.is_empty());
    storage::save_settings(&serde_json::json!({"remember_login": false})).unwrap();
    assert!(!storage::session_path("session.json").exists());
    storage::save_settings(&serde_json::json!({"remember_login": true})).unwrap();
    alice.save_to_file("session.json").unwrap();

    let stale = alice.clone();
    alice.logout();
    assert!(!stale.is_active());
    storage::remove_session().unwrap();
    assert!(stale.save_to_file("session.json").is_err());
    assert!(!storage::session_path("session.json").exists());
    assert!(chat::load_threads_cache(&alice).is_empty());
    storage::save_settings(&serde_json::json!({"remember_login": false})).unwrap();
    assert!(!storage::session_path("session.json").exists());
    drop(instance);
    std::fs::remove_dir_all(root).unwrap();
}
