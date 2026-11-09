//! Synthetic regression fixtures; no live credentials or remote services are used.
use base64::Engine;
use serde_json::{json, Value};
use std::{path::Path, sync::OnceLock};
use zalo_rust::{
    chat::{self, ChatMessage, Thread},
    crypto::{session::E2eeManager, wire},
    e2ee_integration,
    session::{LoginInfo, Session},
    store::{self, MessageStore},
};

fn root() -> &'static Path {
    static ROOT: OnceLock<tempfile::TempDir> = OnceLock::new();
    ROOT.get_or_init(|| {
        let root = tempfile::tempdir().unwrap();
        std::env::set_var("CTDOZ_DATA_DIR", root.path());
        zalo_rust::crypto::store::open_crypto_db().unwrap();
        root
    })
    .path()
}

fn incoming(peer: &str, content: String, encrypted: bool, group: bool) -> Value {
    let message = json!({"msgId":"12001","uidFrom":peer,"idTo":if group {"group-1"}else{"0"},
        "isE2EE":i32::from(encrypted),"isGroup":group,"msgType":"webchat","content":content,"ts":1800000000000i64});
    if group {
        json!({"data":{"groupMsgs":[message]}})
    } else {
        json!({"data":{"msgs":[message]}})
    }
}

#[test]
fn body_shape_and_group_metadata_never_set_direct_encryption_policy() {
    root();
    let header = zalo_rust::crypto::ratchet::MessageHeader {
        msg_type: wire::TYPE_WHISPER,
        dh_public: [5; 32],
        pn: 0,
        n: 0,
    };
    let body = base64::engine::general_purpose::STANDARD
        .encode(wire::serialize_whisper(&header, &[0; 32]));
    let mut threads = Vec::new();
    chat::merge_previews_with_profile(
        &mut threads,
        &incoming("plain-peer", body, false, false),
        Some("policy-owner"),
    );
    assert!(!threads[0].messages[0].is_e2ee);
    assert!(!e2ee_integration::check_thread_e2ee("policy-owner", "plain-peer").unwrap());
    chat::merge_previews_with_profile(
        &mut Vec::new(),
        &incoming("group-sender", "not-base64".into(), true, true),
        Some("policy-owner"),
    );
    assert!(!e2ee_integration::check_thread_e2ee("policy-owner", "group-sender").unwrap());
}

#[tokio::test]
async fn all_direct_content_apis_refuse_required_e2ee_before_network_or_upload() {
    root();
    let mut session = Session::new(false, false, false).unwrap();
    session.login = Some(LoginInfo {
        uin: "send-owner".into(),
        ..Default::default()
    });
    e2ee_integration::remember_e2ee_requirement("send-owner", "send-peer").unwrap();
    let quoted = ChatMessage {
        id: "123".into(),
        client_message_id: "456".into(),
        is_outgoing: true,
        ts: 1800000000000,
        ..Default::default()
    };
    for result in [
        chat::send_quote_message(&session, "send-peer", "fixture quote", &quoted, false).await,
        chat::send_sticker(&session, "send-peer", 1, 2, 1, false).await,
        chat::send_contact_card(
            &session,
            "send-peer",
            false,
            "contact",
            "Fixture",
            "",
            "",
            1,
        )
        .await,
        chat::send_forward_message(&session, "send-peer", false, 1, &json!({}), 1).await,
    ] {
        assert!(result.unwrap_err().to_string().contains("E2EE"));
    }
    let error = chat::send_attachment(
        &session,
        "send-peer",
        false,
        Path::new("missing-fixture-file"),
        "private caption",
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("E2EE"));
}

#[test]
fn receipt_failure_rolls_back_ratchet_prekey_and_identity_together() {
    root();
    let alice = E2eeManager::open("atomic-alice").unwrap();
    let bob = E2eeManager::open("atomic-bob").unwrap();
    let bundle = bob.local_bundle().unwrap().to_remote_bundle().unwrap();
    let before = bob.one_time_prekey_count().unwrap();
    let packet = alice
        .start_session("atomic-bob", &bundle, b"fixture plaintext")
        .unwrap();
    bob.conn().execute_batch("CREATE TRIGGER fail_receive_fixture BEFORE INSERT ON e2ee_received WHEN NEW.profile_id='atomic-bob' BEGIN SELECT RAISE(ABORT,'fixture write failure'); END;").unwrap();
    assert!(bob
        .decrypt_received("atomic-alice", "one", &packet)
        .is_err());
    assert!(bob.load_session("atomic-alice").unwrap().is_none());
    assert!(bob.get_trusted_identity("atomic-alice").unwrap().is_none());
    assert_eq!(bob.one_time_prekey_count().unwrap(), before);
    bob.conn()
        .execute_batch("DROP TRIGGER fail_receive_fixture;")
        .unwrap();
    assert_eq!(
        bob.decrypt_received("atomic-alice", "one", &packet)
            .unwrap()
            .text,
        "fixture plaintext"
    );
    let after = serde_json::to_string(&bob.load_session("atomic-alice").unwrap()).unwrap();
    assert_eq!(
        bob.decrypt_received("atomic-alice", "one", &packet)
            .unwrap()
            .text,
        "fixture plaintext"
    );
    assert_eq!(
        serde_json::to_string(&bob.load_session("atomic-alice").unwrap()).unwrap(),
        after
    );
    assert!(bob
        .decrypt_received("atomic-alice", "another-id", &packet)
        .is_err());
    let mut tampered = packet;
    *tampered.last_mut().unwrap() ^= 1;
    assert!(bob
        .decrypt_received("atomic-alice", "one", &tampered)
        .is_err());
}

#[cfg(feature = "experimental-e2ee")]
#[test]
fn duplicate_delivery_and_failed_history_write_recover_then_delete_private_receipt() {
    root();
    zalo_rust::e2ee_trial::configure(true).unwrap();
    let alice = E2eeManager::open("receive-alice").unwrap();
    let bob = E2eeManager::open("receive-bob").unwrap();
    let bundle = bob.local_bundle().unwrap().to_remote_bundle().unwrap();
    let packet = alice
        .start_session("receive-bob", &bundle, b"fixture plaintext")
        .unwrap();
    let raw = incoming(
        "receive-alice",
        base64::engine::general_purpose::STANDARD.encode(packet),
        true,
        false,
    );
    let mut first = Vec::new();
    chat::merge_previews_with_profile(&mut first, &raw, Some("receive-bob"));
    assert_eq!(first[0].messages[0].text, "fixture plaintext");
    // Simulate a failure/crash before history persistence: only the crypto receipt exists.
    drop(first);
    let mut second = Vec::new();
    chat::merge_previews_with_profile(&mut second, &raw, Some("receive-bob"));
    let mut history = MessageStore::open(zalo_rust::storage::account_file(
        "receive-bob",
        "zalo_history.db",
    ))
    .unwrap();
    store::persist_threads(&mut history, &second).unwrap();
    let mut duplicate = Vec::new();
    chat::merge_previews_with_profile(&mut duplicate, &raw, Some("receive-bob"));
    store::persist_threads(&mut history, &duplicate).unwrap();
    assert_eq!(
        history.messages_for("receive-alice", 10).unwrap()[0].text,
        "fixture plaintext"
    );
    let count = || {
        bob.conn().query_row("SELECT COUNT(*) FROM e2ee_received WHERE profile_id='receive-bob' AND peer_id='receive-alice'",[],|row|row.get::<_,i64>(0)).unwrap()
    };
    assert_eq!(count(), 1);
    history
        .delete_chat_message("receive-alice", &duplicate[0].messages[0])
        .unwrap();
    assert_eq!(count(), 0);
    assert!(history
        .messages_for("receive-alice", 10)
        .unwrap()
        .is_empty());
}

#[test]
fn old_plaintext_is_not_erased_by_a_decryption_failure_placeholder() {
    let mut store = MessageStore::open(":memory:").unwrap();
    let mut thread = Thread {
        id: "legacy-peer".into(),
        messages: vec![ChatMessage {
            id: "legacy-message".into(),
            is_e2ee: true,
            e2ee_identity_changed: true,
            text: "fixture plaintext".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    store::persist_threads(&mut store, std::slice::from_ref(&thread)).unwrap();
    thread.messages[0].text = "[Tin nhắn mã hoá - không giải mã được]".into();
    thread.messages[0].e2ee_identity_changed = false;
    store::persist_threads(&mut store, &[thread]).unwrap();
    let rows = store.messages_for("legacy-peer", 10).unwrap();
    assert_eq!(rows[0].text, "fixture plaintext");
    assert!(rows[0].raw_json.contains("\"e2ee_identity_changed\":true"));
}

#[test]
fn malformed_bundle_ids_and_device_envelopes_are_errors() {
    root();
    let manager = E2eeManager::open("bundle-owner").unwrap();
    let bundle = manager.local_bundle().unwrap().to_export_json();
    for path in ["/registrationId", "/signedPreKey/keyId", "/preKeys/0/keyId"] {
        let mut invalid = bundle.clone();
        *invalid.pointer_mut(path).unwrap() = json!(4294967297u64);
        assert!(zalo_rust::crypto::prekey::parse_bundle(&invalid).is_err());
    }
    let mut missing = bundle.clone();
    missing.as_object_mut().unwrap().remove("registrationId");
    assert!(zalo_rust::crypto::prekey::parse_bundle(&missing).is_err());
    let mut invalid = bundle;
    invalid["signingIdentityKey"] = json!("invalid");
    assert!(zalo_rust::crypto::prekey::parse_bundle(&invalid).is_err());
    for response in [
        json!({"error_code":600}),
        json!({"error_code":0,"data":{}}),
        json!({"error_code":0,"data":{"items":[{}]}}),
    ] {
        assert!(zalo_rust::device::parse_device_list(&response, None).is_err());
    }
    assert!(zalo_rust::device::parse_device_list(
        &json!({"error_code":0,"data":{"items":[]}}),
        None
    )
    .unwrap()
    .is_empty());
}

#[test]
fn remembered_logins_populate_registry_and_selected_account_survives_restart() {
    root();
    let create = |uid: &str| {
        let mut session = Session::new(false, false, false).unwrap();
        session.login = Some(LoginInfo {
            uin: uid.into(),
            display_name: format!("Fixture {uid}"),
            ..Default::default()
        });
        session.set_zpw_sek("synthetic-profile-token".into());
        session.secret_key = Some(vec![7; 32]);
        session.save_to_file("session.json").unwrap();
        session
    };
    create("profile-a");
    let old = create("profile-b");
    let registry = zalo_rust::profiles::ProfileRegistry::load().unwrap();
    let a = registry
        .profiles
        .values()
        .find(|profile| profile.uid == "profile-a")
        .unwrap();
    let b = registry
        .profiles
        .values()
        .find(|profile| profile.uid == "profile-b")
        .unwrap();
    zalo_rust::profiles::activate_for_switch(&old, &a.name).unwrap();
    assert!(!old.is_active());
    assert!(old.save_to_file("session.json").is_err());
    let mut restored = Session::new(false, false, false).unwrap();
    restored.load_from_file("session.json").unwrap();
    assert_eq!(restored.account_key(), "profile-a");
    zalo_rust::profiles::forget_session("profile-a").unwrap();
    assert!(!a.session_file.exists());
    assert!(b.session_file.exists());
}

#[test]
fn locked_notifications_withdraw_pending_content_and_activations() {
    use zalo_rust::desktop_notice as notice;
    notice::account("lock-owner");
    notice::configure(true, true);
    let message = notice::Notice {
        owner: "lock-owner".into(),
        body: "private fixture".into(),
        ..Default::default()
    };
    notice::publish(message.clone(), 7);
    assert!(notice::snapshot().notice.is_some());
    notice::activate(message.clone());
    notice::privacy_lock("lock-owner", true);
    notice::publish(
        notice::Notice {
            owner: "old-account".into(),
            body: "late fixture".into(),
            ..Default::default()
        },
        1,
    );
    notice::count("old-account", 1);
    assert_eq!(notice::snapshot().owner, "lock-owner");
    assert!(notice::snapshot().privacy_locked);
    notice::publish(message.clone(), 7);
    notice::activate(message.clone());
    assert!(notice::snapshot().notice.is_none());
    assert!(notice::take_activation().is_none());
    let mut popup = notice::Popup::default();
    popup.update(&notice::snapshot(), std::time::Instant::now(), true);
    assert!(popup.current.is_none());
    notice::privacy_lock("lock-owner", false);
    assert!(notice::snapshot().notice.is_none());
    notice::publish(message, 7);
    assert!(notice::snapshot().notice.is_some());
}

#[cfg(feature = "experimental-e2ee")]
#[test]
fn trial_logging_checks_readiness_and_drains_on_normal_shutdown() {
    if let Ok(mode) = std::env::var("CTDOZ_LOG_FIXTURE_MODE") {
        if mode == "blocked" {
            assert!(zalo_rust::e2ee_trial::configure(true).is_err());
            assert!(!zalo_rust::e2ee_trial::enabled());
        } else {
            zalo_rust::e2ee_trial::configure(true).unwrap();
            drop(zalo_rust::e2ee_trial::LoggingGuard);
            let log = std::fs::read_to_string(
                zalo_rust::storage::root().join("diagnostics/sync-media.jsonl"),
            )
            .unwrap();
            assert!(log.contains("\"stage\":\"startup\""));
            assert!(log.contains("\"outcome\":\"success\""));
            assert!(log.contains("build_revision"));
        }
        return;
    }
    for mode in ["blocked", "writable"] {
        let dir = tempfile::tempdir().unwrap();
        if mode == "blocked" {
            std::fs::write(dir.path().join("diagnostics"), b"fixture").unwrap();
        }
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "trial_logging_checks_readiness_and_drains_on_normal_shutdown",
                "--nocapture",
            ])
            .env("CTDOZ_LOG_FIXTURE_MODE", mode)
            .env("CTDOZ_DATA_DIR", dir.path())
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "{}",
            String::from_utf8_lossy(&child.stdout)
        );
    }
}

#[test]
#[ignore = "controlled queue merge benchmark; run explicitly with --ignored --nocapture"]
fn queue_merge_lookup_benchmark() {
    for count in [1000usize, 5000, 10000] {
        let base = (0..count)
            .map(|i| Thread {
                id: format!("peer-{i}"),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let messages = (0..1000).map(|i|json!({"msgId":format!("bench-{i}"),"uidFrom":format!("peer-{}",count-1),"idTo":"0","content":"fixture","ts":1800000000000i64+i})).collect::<Vec<_>>();
        let input = json!({"data":{"msgs":messages}});
        let mut samples = Vec::new();
        for _ in 0..7 {
            let mut threads = base.clone();
            let start = std::time::Instant::now();
            chat::merge_previews(&mut threads, &input);
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(
                threads
                    .iter()
                    .map(|thread| thread.messages.len())
                    .sum::<usize>(),
                1000
            );
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{}",
            json!({"task":"queue_merge","threads":count,"messages":1000,"iterations":7,"min_ms":samples[0],"avg_ms":samples.iter().sum::<f64>()/7.0,"p50_ms":samples[3],"p95_ms":samples[6],"p99_ms":samples[6]})
        );
    }
}
