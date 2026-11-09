//! Synthetic audit fixtures. Only loopback traffic is used; no Zalo account is contacted.
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use zalo_rust::{
    chat::{self, ChatMessage, Thread},
    message_model::WireMessageRef,
    session::{LoginInfo, Session},
};

async fn fixture(body: Option<&str>) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    let body = body.map(str::to_owned);
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        loop {
            let accepted =
                tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
            let Ok(Ok((mut stream, _))) = accepted else {
                break;
            };
            let mut request = Vec::new();
            loop {
                let mut block = [0; 8192];
                let n = stream.read(&mut block).await.unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&block[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..end]);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")?
                                .trim()
                                .parse::<usize>()
                                .ok()
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            requests.push(String::from_utf8_lossy(&request).into_owned());
            if let Some(body) = &body {
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
            }
            drop(stream);
        }
        requests
    });
    (address, task)
}
fn session(host: &str) -> Session {
    static NEXT_ACCOUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut s = Session::new(false, false, false).unwrap();
    s.http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    s.login = Some(LoginInfo {
        uin: format!(
            "fixture-{}-{}",
            std::process::id(),
            NEXT_ACCOUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ),
        ..Default::default()
    });
    s.secret_key = Some(vec![7; 32]);
    s.domains.chat = host.into();
    s.domains.group = host.into();
    s.domains.file = host.into();
    s.domains.conversation = host.into();
    s
}
fn quote() -> ChatMessage {
    ChatMessage {
        id: "42".into(),
        sender: "Alice".into(),
        client_message_id: "777".into(),
        ts: 1_800_000_000_123,
        wire_ref: WireMessageRef::from_raw(
            &serde_json::json!({"msgId":"42","cliMsgId":"777","uidFrom":"456"}),
        ),
        text: "original".into(),
        ..Default::default()
    }
}
fn params(request: &str) -> serde_json::Value {
    let form = request.split("\r\n\r\n").nth(1).unwrap();
    let url = reqwest::Url::parse(&format!("http://fixture.invalid/?{form}")).unwrap();
    let encrypted = url
        .query_pairs()
        .find(|(key, _)| key == "params")
        .unwrap()
        .1
        .into_owned();
    serde_json::from_str(&zalo_rust::crypto::decode_aes(&[7; 32], &encrypted).unwrap()).unwrap()
}
#[tokio::test]
async fn unicode_html_comment_does_not_abort_link_scraping() {
    let (url, task) = fixture(Some("<!--你好--><title>Tiếng Việt</title>")).await;
    let metadata = chat::scrape_opengraph_metadata(&url).await.unwrap();
    assert_eq!(metadata.title, "Tiếng Việt");
    task.await.unwrap();
}
#[tokio::test]
async fn uncertain_quote_delivery_never_issues_a_second_mutation() {
    let (url, task) = fixture(None).await;
    assert!(
        chat::send_quote_message(&session(&url), "789", "reply", &quote(), true)
            .await
            .is_err()
    );
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /api/group/quote"));
}
#[tokio::test]
async fn uncertain_link_delivery_never_falls_back_to_sms() {
    let (url, task) = fixture(None).await;
    assert!(chat::send_link_message(
        &session(&url),
        "456",
        "link",
        &chat::LinkMetadata {
            url: "https://example.invalid".into(),
            ..Default::default()
        },
        false
    )
    .await
    .is_err());
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /api/message/link"));
}
#[tokio::test]
async fn quote_request_preserves_sender_and_client_identity_instead_of_display_or_time() {
    let (url, task) = fixture(Some(r#"{"error_code":0,"data":{"msgId":"99"}}"#)).await;
    chat::send_quote_message(&session(&url), "789", "reply", &quote(), true)
        .await
        .unwrap();
    let requests = task.await.unwrap();
    let body = params(&requests[0]);
    assert_eq!(body["qmsgOwner"], "456");
    assert_eq!(body["qmsgCliId"], 777);
}
#[tokio::test]
async fn missing_or_opaque_receipts_never_confirm_quote_or_recall() {
    for body in [
        r#"{}"#,
        r#"{"error_code":0,"data":null}"#,
        r#"{"error_code":"114","data":{}}"#,
        r#"{"error_code":0,"data":"not-decryptable"}"#,
    ] {
        let (url, task) = fixture(Some(body)).await;
        assert!(
            chat::send_quote_message(&session(&url), "789", "reply", &quote(), true)
                .await
                .is_err()
        );
        assert_eq!(task.await.unwrap().len(), 1);
    }
    let (url, task) = fixture(Some("{}")).await;
    let mut msg = quote();
    msg.is_outgoing = true;
    assert!(chat::recall_message(&session(&url), "789", true, &msg)
        .await
        .is_err());
    let requests = task.await.unwrap();
    assert_eq!(params(&requests[0])["cliMsgIdUndo"], "777");
}
#[tokio::test]
async fn direct_media_forward_is_rejected_before_contacting_a_server() {
    // Media is forwarded only through the outgoing queue, which re-uploads the file.
    let s = session("http://127.0.0.1:1");
    let msg = ChatMessage {
        text: "[Tập tin] report.pdf".into(),
        link_url: Some("https://example.invalid/report.pdf".into()),
        ..Default::default()
    };
    assert!(chat::can_forward_message(&msg));
    assert!(chat::forwardable_media(&msg));
    assert!(chat::forward_message(&s, "456", false, &msg)
        .await
        .unwrap_err()
        .to_string()
        .contains("Chưa hỗ trợ"));
}
#[test]
fn read_cursor_and_policy_survive_ingestion_and_thread_merge() {
    let mut threads = vec![Thread {
        id: "456".into(),
        ..Default::default()
    }];
    chat::merge_previews(
        &mut threads,
        &serde_json::json!({"data":{"clearUnreads":[{"idTo":"456","lastMsgId":"10"}],"msgs":[
            {"msgId":"11","uidFrom":"456","idTo":"0","content":"new","ts":1700000000000i64},
            {"msgId":"12","uidFrom":"456","idTo":"0","content":"silent","ts":1700000000001i64,"paramsExt":{"countUnread":0}}
        ]}}),
    );
    assert_eq!(threads[0].unread.count(), 1);
    let mut target = Thread {
        id: "456".into(),
        ..Default::default()
    };
    chat::merge_thread_into(&mut target, threads.remove(0), true);
    assert_eq!(target.unread.last_seen_id.as_deref(), Some("10"));
    assert_eq!(target.unread.count(), 1);
    chat::merge_previews(
        &mut threads,
        &serde_json::json!({"data":{"clearUnreads":[{"idTo":"456","lastMsgId":"12"}]}}),
    );
    chat::merge_thread_into(&mut target, threads.remove(0), true);
    assert_eq!(target.unread.count(), 0);
    assert_eq!(target.unread.last_seen_id.as_deref(), Some("12"));
}
#[tokio::test]
async fn original_above_64_mib_is_read_in_bounded_chunks() {
    let path = std::env::temp_dir().join(format!(
        "ctdoz-upload-limit-{}",
        zalo_rust::param_cipher::random_hex(12)
    ));
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(65 * 1024 * 1024 + 17).unwrap();
    drop(file);
    let mut source = zalo_rust::upload::Source::open(&path).await.unwrap();
    assert_eq!(source.size, 65 * 1024 * 1024 + 17);
    let mut total = 0;
    while total < source.size {
        let chunk = source.next_chunk().await.unwrap();
        assert!(chunk.len() <= zalo_rust::upload::CHUNK);
        assert!(chunk.iter().all(|byte| *byte == 0));
        total += chunk.len() as u64;
    }
    assert_eq!(total, source.size);
    source.verify().await.unwrap();
    drop(source);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn photo_upload_with_no_ack_does_not_send_or_report_success() {
    let path = std::env::temp_dir().join(format!(
        "ctdoz-photo-{}.png",
        zalo_rust::param_cipher::random_hex(12)
    ));
    image::RgbaImage::new(1, 1).save(&path).unwrap();
    let (url, task) = fixture(Some("{}")).await;
    let result = chat::send_attachment(&session(&url), "456", false, &path, "").await;
    std::fs::remove_file(path).unwrap();
    assert!(result.is_err());
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].starts_with("POST /api/message/photo_original/upload"));
}

#[tokio::test]
async fn retry_keeps_client_identity_and_clears_job_only_after_ack() {
    use zalo_rust::outgoing::{self, Payload};
    let message = outgoing::pending("retry fixture".into(), false, Payload::Text);
    let id = message.client_message_id.clone();
    let (url, task) = fixture(Some("{\"error_code\":-1,\"data\":{}}")).await;
    assert!(outgoing::send(&session(&url), "456", &message)
        .await
        .is_err());
    let failed = task.await.unwrap();
    assert_eq!(
        params(&failed[0])["clientId"].as_u64().unwrap().to_string(),
        id
    );
    let (url, task) = fixture(Some("{\"error_code\":0,\"data\":{\"msgId\":\"999\"}}")).await;
    let sent = outgoing::send(&session(&url), "456", &message)
        .await
        .unwrap();
    assert_eq!(sent.client_message_id, id);
    assert_eq!(sent.id, "999");
    assert!(sent.outgoing_job.is_none());
    assert_eq!(
        sent.delivery,
        zalo_rust::message_model::DeliveryState::ServerAccepted
    );
    let retry = task.await.unwrap();
    assert_eq!(
        params(&retry[0])["clientId"],
        params(&failed[0])["clientId"]
    );
}

#[tokio::test]
async fn changed_file_is_detected_before_send() {
    let path = std::env::temp_dir().join(format!(
        "stream-change-{}",
        zalo_rust::param_cipher::random_hex(12)
    ));
    std::fs::write(&path, b"original").unwrap();
    let source = zalo_rust::upload::Source::open(&path).await.unwrap();
    std::fs::write(&path, b"changed file").unwrap();
    assert!(source.verify().await.is_err());
    drop(source);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn streaming_upload_preserves_chunk_order_checksum_and_final_ack() {
    let path = std::env::temp_dir().join(format!(
        "stream-fixture-{}.bin",
        zalo_rust::param_cipher::random_hex(12)
    ));
    let mut bytes = vec![b'A'; zalo_rust::upload::CHUNK];
    bytes.extend(vec![b'B'; zalo_rust::upload::CHUNK]);
    bytes.extend([b'C'; 17]);
    std::fs::write(&path, &bytes).unwrap();
    let body = "{\"error_code\":0,\"data\":{\"fileId\":\"7\",\"fileUrl\":\"https://fixture.invalid/file\",\"msgId\":\"999\"}}";
    let (url, task) = fixture(Some(body)).await;
    let (_, message) = chat::send_attachment(&session(&url), "456", false, &path, "caption")
        .await
        .unwrap();
    let requests = task.await.unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(requests.len(), 4);
    for (index, request) in requests[..3].iter().enumerate() {
        let first_line = request.lines().next().unwrap();
        let target = first_line.split_whitespace().nth(1).unwrap();
        let query = reqwest::Url::parse(&format!("http://fixture.invalid{target}")).unwrap();
        let encrypted = query
            .query_pairs()
            .find(|(key, _)| key == "params")
            .unwrap()
            .1
            .into_owned();
        let values: serde_json::Value =
            serde_json::from_str(&zalo_rust::crypto::decode_aes(&[7; 32], &encrypted).unwrap())
                .unwrap();
        assert_eq!(values["chunkId"], index + 1);
        assert_eq!(values["totalChunk"], 3);
        assert_eq!(values["totalSize"], bytes.len());
        assert_eq!(
            values["clientId"].as_u64().unwrap().to_string(),
            message.client_message_id
        );
        let multipart = request.split_once("\r\n\r\n").unwrap().1;
        let data = multipart.split_once("\r\n\r\n").unwrap().1;
        let chunk = data.split_once("\r\n--").unwrap().0.as_bytes();
        let start = index * zalo_rust::upload::CHUNK;
        assert_eq!(
            chunk,
            &bytes[start..(start + zalo_rust::upload::CHUNK).min(bytes.len())]
        );
    }
    use md5::{Digest, Md5};
    assert_eq!(
        params(&requests[3])["checksum"],
        format!("{:x}", Md5::digest(&bytes))
    );
    assert_eq!(message.id, "999");
}
#[tokio::test]
async fn forward_message_body_matches_zalo_pc_forward_message() {
    let ok = r#"{"error_code":0,"data":{"msgId":"9"}}"#;
    let info =
        serde_json::json!({"videoUrl":"https://video.zdn.vn/v.mp4","thumbUrl":"","duration":1});
    let (url, task) = fixture(Some(ok)).await;
    chat::send_forward_message(&session(&url), "456", false, 5, &info, 77)
        .await
        .unwrap();
    let requests = task.await.unwrap();
    assert!(requests[0].starts_with("POST /api/message/forward?"));
    let body = params(&requests[0]);
    assert_eq!(body["toId"], "456");
    assert_eq!(body["msgType"], 5);
    assert_eq!(body["clientId"], 77);
    let sent: serde_json::Value = serde_json::from_str(body["msgInfo"].as_str().unwrap()).unwrap();
    assert_eq!(sent, info);
    let (url, task) = fixture(Some(ok)).await;
    chat::send_forward_message(&session(&url), "g789", true, 2, &info, 78)
        .await
        .unwrap();
    let requests = task.await.unwrap();
    assert!(requests[0].starts_with("POST /api/group/forward?"));
    let body = params(&requests[0]);
    assert_eq!(
        (body["grid"].as_str(), body["visibility"].as_i64()),
        (Some("789"), Some(0))
    );
    assert!(body.get("toId").is_none());
}

#[tokio::test]
async fn uncertain_cloned_media_delivery_never_reuploads_or_sends_twice() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut paths = Vec::new();
        while let Ok(Ok((mut stream, _))) = tokio::time::timeout(
            if paths.len() < 4 {
                Duration::from_secs(5)
            } else {
                Duration::from_millis(300)
            },
            listener.accept(),
        )
        .await
        {
            let mut request = Vec::new();
            loop {
                let mut block = [0; 8192];
                let n = stream.read(&mut block).await.unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&block[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let length = String::from_utf8_lossy(&request[..end])
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length:")?
                                .trim()
                                .parse::<usize>()
                                .ok()
                        })
                        .unwrap_or(0);
                    if request.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            paths.push(
                String::from_utf8_lossy(&request)
                    .lines()
                    .next()
                    .unwrap()
                    .to_owned(),
            );
            let body = match paths.len() {
                1 | 2 => Some(r#"{"error_code":0,"data":{"convers":[]}}"#),
                3 => Some(
                    r#"{"error_code":0,"data":{"normal":"https://photo.zdn.vn/copy","thumb":"https://photo.zdn.vn/thumb"}}"#,
                ),
                4 => None, // The send reached the server, but its acknowledgement was lost.
                _ => Some(r#"{"error_code":1,"data":{}}"#),
            };
            if let Some(body) = body {
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
            }
        }
        paths
    });
    let mut original = ChatMessage {
        id: "42".into(),
        kind: zalo_rust::message_model::MessageKind::Photo,
        // A re-upload would fail validation locally, then try renewing the source.
        link_url: Some("https://unsupported.invalid/image".into()),
        ..Default::default()
    };
    chat::set_forward_source(&mut original, "789", false);
    let pending = zalo_rust::outgoing::pending(
        "forward".into(),
        false,
        zalo_rust::outgoing::Payload::Forward(Box::new(original)),
    );
    assert!(zalo_rust::outgoing::send(&session(&url), "456", &pending)
        .await
        .is_err());
    let paths = server.await.unwrap();
    assert_eq!(
        paths.len(),
        4,
        "A send failure must not restart media preparation"
    );
    assert!(paths[2].starts_with("POST /api/message/renewlink"));
    assert!(paths[3].starts_with("POST /api/message/forward"));
}

#[test]
fn audit_a16_websocket_queue_backpressure_and_flow_control() {
    use serde_json::json;
    use std::time::Instant;

    // Test that PullQueues handles flow control backpressure
    let mut queues = zalo_rust::socket::queue_receiver::PullQueues::new(10);
    let key = (510, 0);
    let now = Instant::now();
    queues.register(key, "req-1".into(), now, None);

    let page_payload = json!({
        "data": {
            "lastActionId": 999,
            "more": 1
        }
    });

    // 1. When forward queue is congested (has_capacity = false), page is throttled
    let throttled = queues.page_with_capacity(key, &page_payload, now, false);
    assert!(
        throttled.is_none(),
        "Must throttle pull when downstream buffer has no capacity"
    );
    assert!(queues.is_throttled(key), "Queue must be marked throttled");

    // 2. due_with_capacity(false) will NOT resume throttled queues
    assert!(
        queues.due_with_capacity(now, false).is_empty(),
        "Must not resume while capacity is unavailable"
    );

    // 3. When capacity is restored (has_capacity = true), due_with_capacity resumes the throttled queue
    let resumed = queues.due_with_capacity(now, true);
    assert_eq!(
        resumed.len(),
        1,
        "Must resume throttled queue once capacity is restored"
    );
    assert_eq!(resumed[0].last, json!(999));
    assert!(!queues.is_throttled(key));
}

#[test]
fn audit_a20_multi_account_profile_isolation_and_rollback_preservation() {
    use zalo_rust::account_controls::Impact;

    // 1. Account switch impact must invalidate all per-account state
    let impact = Impact::for_account_switch();
    assert!(impact.account, "Must invalidate account boundary");
    assert!(impact.profiles, "Must invalidate cached profiles");
    assert!(impact.privacy, "Must invalidate privacy settings");
    assert!(impact.relations, "Must invalidate friends/relations");
    assert!(impact.hidden, "Must invalidate hidden conversations");
    assert!(impact.all_groups, "Must invalidate all group memberships");

    // 2. Profile registry isolation
    let mut reg = zalo_rust::profiles::ProfileRegistry::default();
    reg.add_profile(zalo_rust::profiles::Profile {
        name: "account_a".into(),
        uid: "111".into(),
        phone: Some("0901234567".into()),
        display_name: "Account A".into(),
        avatar_url: None,
        last_active: 100,
        session_file: std::path::PathBuf::from("a.json"),
    });
    reg.add_profile(zalo_rust::profiles::Profile {
        name: "account_b".into(),
        uid: "222".into(),
        phone: Some("0907654321".into()),
        display_name: "Account B".into(),
        avatar_url: None,
        last_active: 200,
        session_file: std::path::PathBuf::from("b.json"),
    });
    reg.set_active("account_a").unwrap();
    assert_eq!(reg.active().unwrap().uid, "111");

    reg.set_active("account_b").unwrap();
    assert_eq!(reg.active().unwrap().uid, "222");

    // Non-existent profile fails without corrupting active profile
    assert!(reg.set_active("non_existent").is_err());
    assert_eq!(reg.active().unwrap().uid, "222");
}

#[test]
fn audit_a33_http2_header_injection_prevention() {
    use zalo_rust::endpoints::{has_header_injection, sanitize_header_value};

    // CRLF and null byte detection
    assert!(has_header_injection("value\r\nEvil-Header: 1"));
    assert!(has_header_injection("value\r"));
    assert!(has_header_injection("value\n"));
    assert!(has_header_injection("value\0null"));
    assert!(!has_header_injection(
        "Mozilla/5.0 (Windows NT 10.0; ARM64)"
    ));

    // Sanitization strips forbidden characters
    assert_eq!(
        sanitize_header_value("evil\r\nCustom: injected\0value"),
        "evilCustom: injectedvalue"
    );
    assert_eq!(
        sanitize_header_value("clean_header_value_123"),
        "clean_header_value_123"
    );
}

#[test]
fn audit_a35_ipv6_link_local_and_site_local_guards() {
    use std::net::IpAddr;
    use zalo_rust::chat::is_safe_opengraph_url;
    use zalo_rust::endpoints::is_private_or_loopback_ip;

    // IPv6 link-local (fe80::/10)
    let fe80_1 = "fe80::1".parse::<IpAddr>().unwrap();
    let fe80_high = "fe80:0:0:0:ffff:ffff:ffff:ffff".parse::<IpAddr>().unwrap();
    assert!(is_private_or_loopback_ip(fe80_1));
    assert!(is_private_or_loopback_ip(fe80_high));

    // IPv6 site-local (fec0::/10, RFC 3879)
    let fec0_1 = "fec0::1".parse::<IpAddr>().unwrap();
    let fec0_high = "fec0:0:0:0:ffff:ffff:ffff:ffff".parse::<IpAddr>().unwrap();
    assert!(is_private_or_loopback_ip(fec0_1));
    assert!(is_private_or_loopback_ip(fec0_high));

    // IPv6 multicast (ff00::/8)
    let ff02_1 = "ff02::1".parse::<IpAddr>().unwrap();
    assert!(is_private_or_loopback_ip(ff02_1));

    // Verify OpenGraph URL security validator rejects them
    let url_fe80 = reqwest::Url::parse("http://[fe80::1]/").unwrap();
    assert!(!is_safe_opengraph_url(&url_fe80, false));

    let url_fec0 = reqwest::Url::parse("http://[fec0::1]/private").unwrap();
    assert!(!is_safe_opengraph_url(&url_fec0, false));

    let url_fec0_high = reqwest::Url::parse("http://[fec0::ffff]/metadata").unwrap();
    assert!(!is_safe_opengraph_url(&url_fec0_high, false));
}
