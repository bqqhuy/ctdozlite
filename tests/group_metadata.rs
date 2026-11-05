//! Loopback protocol fixture with an isolated account cache.
use serde_json::{json, Value};
use zalo_rust::{chat::get_groups, session::Session};
#[tokio::test]
async fn requests_missing_metadata_with_zero_version_and_decodes_group_name() {
    use std::io::{Read, Write};
    let isolated = tempfile::tempdir().unwrap();
    std::env::set_var("CTDOZ_DATA_DIR", isolated.path());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for step in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut input = Vec::new();
            let header_end = loop {
                let mut b = [0; 1024];
                let n = stream.read(&mut b).unwrap();
                assert!(n > 0);
                input.extend_from_slice(&b[..n]);
                if let Some(end) = input.windows(4).position(|s| s == b"\r\n\r\n") {
                    break end + 4;
                }
            };
            let header = String::from_utf8_lossy(&input[..header_end]).to_string();
            if step == 1 {
                assert!(header.starts_with("POST /api/group/getmg-v2?"));
                let length: usize = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|s| s.trim().parse().unwrap())
                    })
                    .unwrap();
                while input.len() < header_end + length {
                    let mut b = [0; 1024];
                    let n = stream.read(&mut b).unwrap();
                    assert!(n > 0);
                    input.extend_from_slice(&b[..n]);
                }
                let form = String::from_utf8_lossy(&input[header_end..]);
                let url = reqwest::Url::parse(&format!("http://fixture/?{form}")).unwrap();
                let params = url
                    .query_pairs()
                    .find(|(key, _)| key == "params")
                    .unwrap()
                    .1
                    .to_string();
                let decoded = zalo_rust::crypto::decode_aes(&[0; 32], &params).unwrap();
                let body: Value = serde_json::from_str(&decoded).unwrap();
                let versions: Value =
                    serde_json::from_str(body["gridVerMap"].as_str().unwrap()).unwrap();
                assert_eq!(versions, json!({"123":0}));
            }
            let data = if step == 0 {
                json!({"gridVerMap":{"123":999}})
            } else {
                json!({"gridInfoMap":{"123":{"name":"Nhóm ARM64","avt":"https://example.invalid/avatar.png"}}})
            };
            let body = json!({"error_code":0,"data":data.to_string()}).to_string();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    let mut session = Session::new(false, false, false).unwrap();
    session.http = reqwest::Client::builder().no_proxy().build().unwrap();
    session.domains.group = format!("http://{addr}");
    session.secret_key = Some(vec![0; 32]);
    session.login = Some(zalo_rust::session::LoginInfo {
        uin: "fixture-user".into(),
        zpw_enk: String::new(),
        zpw_sek: String::new(),
        display_name: "Fixture".into(),
        send2me_id: String::new(),
        avatar: String::new(),
    });
    let groups = get_groups(&session).await.unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].name, "Nhóm ARM64");
    server.join().unwrap();
}
