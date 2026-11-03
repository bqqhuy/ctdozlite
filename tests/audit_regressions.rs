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
    let mut s = Session::new(false, false, false).unwrap();
    s.http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    s.login = Some(LoginInfo {
        uin: "123".into(),
        ..Default::default()
    });
    s.secret_key = Some(vec![7; 32]);
    s.domains.chat = host.into();
    s.domains.group = host.into();
    s.domains.file = host.into();
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
