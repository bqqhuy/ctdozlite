//! Explicitly opted-in E3 experiment. Never claims Zalo interoperability.

use anyhow::Result;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Enable the experiment only in a specially compiled binary and for this launch.
pub fn configure(requested: bool) -> Result<()> {
    anyhow::ensure!(
        !requested || cfg!(feature = "experimental-e2ee"),
        "Bản này chưa bật experimental-e2ee khi build"
    );
    ENABLED.store(requested, Ordering::Release);
    if requested {
        Trace::new("", "", Stage::Startup, 0).finish(true);
        if !crate::transfer_diagnostics::flush(std::time::Duration::from_secs(2)) {
            ENABLED.store(false, Ordering::Release);
            anyhow::bail!("Không ghi được log E2EE; kiểm tra thư mục diagnostics trước khi thử");
        }
    }
    Ok(())
}

/// Whether this process explicitly opted into the unverified protocol experiment.
pub fn enabled() -> bool {
    cfg!(feature = "experimental-e2ee") && ENABLED.load(Ordering::Acquire)
}

/// Persist pending diagnostics after the GUI/CLI has stopped, with a bounded wait.
pub struct LoggingGuard;
impl Drop for LoggingGuard {
    fn drop(&mut self) {
        // Shutdown is bounded even when storage is unavailable.
        let _ = crate::transfer_diagnostics::flush(std::time::Duration::from_secs(2));
    }
}

/// Inspect diagnostic writer health without blocking the UI.
pub fn logging_healthy() -> bool {
    crate::transfer_diagnostics::healthy()
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Stage {
    Startup,
    Contract,
    Policy,
    Decode,
    Load,
    #[cfg(feature = "experimental-e2ee")]
    FetchBundle,
    #[cfg(feature = "experimental-e2ee")]
    ParseBundle,
    #[cfg(feature = "experimental-e2ee")]
    UploadBundle,
    #[cfg(feature = "experimental-e2ee")]
    EncryptPersist,
    #[cfg(feature = "experimental-e2ee")]
    EncodeRequest,
    #[cfg(feature = "experimental-e2ee")]
    Send,
    #[cfg(feature = "experimental-e2ee")]
    Acknowledgement,
    DecryptPersist,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Started,
    Success,
    Failed,
    Cancelled,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum Failure {
    Timeout,
    Network,
    Database,
    Authentication,
    Replay,
    Signature,
    MissingSession,
    MissingPrekey,
    Other,
}

fn classify(error: &anyhow::Error) -> Failure {
    if let Some(error) = error.downcast_ref::<reqwest::Error>() {
        return if error.is_timeout() {
            Failure::Timeout
        } else {
            Failure::Network
        };
    }
    if error.downcast_ref::<rusqlite::Error>().is_some() {
        return Failure::Database;
    }
    // Match known internal crypto diagnostics; never serialize arbitrary error text.
    for cause in error.chain() {
        let text = cause.to_string();
        if text.contains("MAC verification failed") {
            return Failure::Authentication;
        }
        if text.contains("replay attack") {
            return Failure::Replay;
        }
        if text.contains("Chữ ký signedPreKey") {
            return Failure::Signature;
        }
        if text.contains("Chưa có phiên E2EE") {
            return Failure::MissingSession;
        }
        if text.contains("prekey") && text.contains("không tồn tại") {
            return Failure::MissingPrekey;
        }
    }
    Failure::Other
}

#[derive(Clone, Serialize)]
pub(crate) struct Diagnostic {
    run: u64,
    attempt: u64,
    account_tag: u64,
    peer_tag: u64,
    stage: Stage,
    outcome: Outcome,
    elapsed_ms: u64,
    bytes: usize,
    wire_type: Option<u8>,
    counter: Option<u32>,
    previous_chain_count: Option<u32>,
    server_code: Option<i64>,
    http_status: Option<u16>,
    response_shape: u8,
    experimental: bool,
    version: &'static str,
    build_revision: &'static str,
    arch: &'static str,
    failure: Option<Failure>,
}

fn correlation(account: &str, peer: &str) -> (u64, u64, u64) {
    use sha2::{Digest, Sha256};
    static SALT: std::sync::LazyLock<Option<[u8; 32]>> = std::sync::LazyLock::new(|| {
        use aws_lc_rs::rand::{SecureRandom, SystemRandom};
        let mut bytes = [0; 32];
        SystemRandom::new().fill(&mut bytes).ok().map(|()| bytes)
    });
    // Loss of correlation is preferable to unsalted, enumerable identity hashes.
    let Some(salt) = SALT.as_ref() else {
        return (0, 0, 0);
    };
    let hash = |domain: u8, parts: &[&str]| {
        let mut digest = Sha256::new();
        digest.update(salt);
        digest.update([domain]);
        for part in parts {
            digest.update((part.len() as u64).to_le_bytes());
            digest.update(part.as_bytes());
        }
        let bytes = digest.finalize();
        u64::from_le_bytes(bytes[..8].try_into().unwrap_or_default())
    };
    (hash(0, &[]), hash(1, &[account]), hash(2, &[account, peer]))
}

/// An allowlisted trace: never accepts payloads, URLs, keys or arbitrary errors.
pub(crate) struct Trace {
    event: Diagnostic,
    start: std::time::Instant,
    finished: bool,
}

impl Trace {
    pub(crate) fn new(account: &str, peer: &str, stage: Stage, bytes: usize) -> Self {
        let (run, account_tag, peer_tag) = correlation(account, peer);
        let trace = Self {
            event: Diagnostic {
                run,
                attempt: NEXT.fetch_add(1, Ordering::Relaxed),
                account_tag,
                peer_tag,
                stage,
                outcome: Outcome::Started,
                elapsed_ms: 0,
                bytes,
                wire_type: None,
                counter: None,
                previous_chain_count: None,
                server_code: None,
                http_status: None,
                response_shape: 0,
                experimental: enabled(),
                version: crate::PRODUCT_VERSION,
                build_revision: option_env!("GITHUB_SHA").unwrap_or("local"),
                arch: std::env::consts::ARCH,
                failure: None,
            },
            start: std::time::Instant::now(),
            finished: false,
        };
        trace.record();
        trace
    }

    fn record(&self) {
        crate::transfer_diagnostics::e2ee(&self.event);
    }

    pub(crate) fn stage(&mut self, stage: Stage) {
        self.event.stage = stage;
        self.event.outcome = Outcome::Started;
        self.event.server_code = None;
        self.event.response_shape = 0;
        self.event.elapsed_ms = self.start.elapsed().as_millis() as u64;
        self.record();
    }

    pub(crate) fn envelope(&mut self, decoded: &crate::crypto::wire::DecodedMessage, bytes: usize) {
        use crate::crypto::wire::DecodedMessage;
        let header = match decoded {
            DecodedMessage::PreKey(message) => &message.header,
            DecodedMessage::Whisper(message) => &message.header,
        };
        self.event.bytes = bytes;
        self.event.wire_type = Some(header.msg_type);
        self.event.counter = Some(header.n);
        self.event.previous_chain_count = Some(header.pn);
        self.record();
    }

    #[cfg(feature = "experimental-e2ee")]
    fn response(&mut self, value: &serde_json::Value) {
        self.event.server_code = value
            .get("error_code")
            .and_then(crate::api_response::integer);
        // Shape flags reveal presence/type only, never field names or values.
        self.event.response_shape = u8::from(value.is_object())
            | (u8::from(value.get("error_code").is_some()) << 1)
            | (u8::from(value.get("data").is_some_and(serde_json::Value::is_object)) << 2);
        self.record();
    }

    pub(crate) fn finish(mut self, success: bool) {
        self.event.outcome = if success {
            Outcome::Success
        } else {
            Outcome::Failed
        };
        self.event.elapsed_ms = self.start.elapsed().as_millis() as u64;
        self.record();
        self.finished = true;
    }

    pub(crate) fn finish_result<T>(mut self, result: &Result<T>) {
        self.event.failure = result.as_ref().err().map(classify);
        if let Err(error) = result {
            self.event.http_status = crate::session::http_failure_status(error);
            if self.event.server_code.is_none() {
                self.event.server_code = error.chain().find_map(|cause| {
                    let text = cause.to_string();
                    text.strip_prefix("server error ")?
                        .split(':')
                        .next()?
                        .trim()
                        .parse()
                        .ok()
                });
            }
        }
        self.finish(result.is_ok());
    }
}

impl Drop for Trace {
    fn drop(&mut self) {
        if !self.finished {
            self.event.outcome = Outcome::Cancelled;
            self.event.elapsed_ms = self.start.elapsed().as_millis() as u64;
            self.record();
        }
    }
}

#[cfg(feature = "experimental-e2ee")]
mod experiment {
    use super::*;
    use crate::{crypto::prekey::parse_bundle, e2ee_integration, session::Session};
    use serde_json::{json, Value};

    async fn request(s: &Session, path: &str, body: &Value) -> Result<Value> {
        use crate::{
            param_cipher::ParamCipher,
            session::{urlencode, ZPW_TYPE, ZPW_VER},
        };
        let cipher = ParamCipher::new(ZPW_TYPE, &s.imei, crate::login::unix_now())?;
        // Endpoint tags follow the standard suffix rule; acceptance here remains unverified.
        let tag = path
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .replace('-', "")
            .to_lowercase();
        let envelope = cipher.build_body(ZPW_TYPE, ZPW_VER, body, &tag)?;
        let form = envelope
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("Không tạo được yêu cầu E2EE"))?
            .iter()
            .map(|(key, value)| format!("{key}={}", urlencode(value.as_str().unwrap_or_default())))
            .collect::<Vec<_>>()
            .join("&");
        let url = format!(
            "{}?{}",
            s.resolve_service_url("chat", path),
            s.common_params()
        );
        let response = s.post_form(&url, form).await?;
        Ok(s.maybe_decrypt(response, cipher.key().as_bytes()).await)
    }

    /// Try the inferred one-to-one text contract once; never retry uncertain delivery.
    pub(crate) async fn send(
        s: &Session,
        profile: &str,
        peer: &str,
        text: &str,
        id: u64,
        trace: &mut Trace,
    ) -> Result<Value> {
        anyhow::ensure!(enabled(), "Chưa bật chế độ E2EE thử nghiệm");
        anyhow::ensure!(
            logging_healthy(),
            "Log E2EE đang lỗi; chưa gửi thử để tránh mất bằng chứng"
        );
        anyhow::ensure!(
            profile == s.account_key() && !profile.is_empty() && !peer.is_empty(),
            "Sai tài khoản E2EE"
        );
        trace.stage(Stage::Load);
        let account = profile.to_owned();
        let recipient = peer.to_owned();
        let exists = tokio::task::spawn_blocking(move || {
            let _guard = e2ee_integration::lock_e2ee()?;
            let manager = e2ee_integration::get_manager(&account)?;
            // Reserve before any network operation, including prekey upload.
            super::reserve(manager.conn(), &account, &recipient, id)?;
            manager.load_session(&recipient).map(|v| v.is_some())
        })
        .await??;
        let remote = if exists {
            None
        } else {
            trace.stage(Stage::FetchBundle);
            let response = request(
                s,
                "/api/e2ee/pc/prekey/get",
                &json!({"toid":peer,"clientId":s.imei}),
            )
            .await?;
            trace.response(&response);
            trace.stage(Stage::ParseBundle);
            crate::chat::confirmed_mutation(response.clone())?;
            let bundle = parse_bundle(&response)?;
            anyhow::ensure!(
                crate::crypto::session::verify_signed_prekey(&bundle),
                "Chữ ký signedPreKey của đối phương không hợp lệ"
            );
            let account = profile.to_owned();
            let local = tokio::task::spawn_blocking(move || {
                let _guard = e2ee_integration::lock_e2ee()?;
                Ok::<_, anyhow::Error>(
                    e2ee_integration::get_manager(&account)?
                        .local_bundle()?
                        .to_export_json(),
                )
            })
            .await??;
            trace.stage(Stage::UploadBundle);
            let mut local = local;
            local["clientId"] = json!(s.imei);
            let response = request(s, "/api/e2ee/pc/prekey/update", &local).await?;
            trace.response(&response);
            crate::chat::confirmed_mutation(response)?;
            Some(bundle)
        };
        trace.stage(Stage::EncryptPersist);
        let account = profile.to_owned();
        let recipient = peer.to_owned();
        let plaintext = text.to_owned();
        let ciphertext = tokio::task::spawn_blocking(move || {
            let _guard = e2ee_integration::lock_e2ee()?;
            let manager = e2ee_integration::get_manager(&account)?;
            manager.encrypt(&recipient, remote.as_ref(), plaintext.as_bytes())
        })
        .await??;
        trace.stage(Stage::EncodeRequest);
        trace.envelope(
            &crate::crypto::wire::deserialize(&ciphertext)?,
            ciphertext.len(),
        );
        use base64::Engine;
        let b64_wire = base64::engine::general_purpose::STANDARD.encode(&ciphertext);
        let list_params = json!([
            {
                "toId": peer,
                "platform": 1,
                "message": b64_wire,
                "e2ee": {
                    "ver": 1,
                    "iid": 1,
                    "sender_iid": 1,
                }
            }
        ]);
        let body = json!({
            "toid": peer,
            "threadId": peer,
            "destType": 3,
            "message": b64_wire,
            "listParams": list_params,
            "clientId": id,
            "imei": s.imei,
            "isE2EE": 1,
        });
        trace.stage(Stage::Send);
        let response = request(s, "/api/e2ee/pc/message/sms", &body).await?;
        trace.stage(Stage::Acknowledgement);
        trace.response(&response);
        crate::chat::confirmed_send(response)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[tokio::test]
        async fn inferred_request_uses_encoded_params_and_preserves_server_rejection() {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let host = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move {
                let (mut stream, _) =
                    tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    headers.push(stream.read_u8().await.unwrap());
                    assert!(headers.len() < 16_384);
                }
                let headers = String::from_utf8(headers).unwrap();
                assert!(headers.starts_with("POST /api/e2ee/pc/prekey/get?"));
                let size = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                assert!(size < 16_384);
                let mut body = vec![0; size];
                stream.read_exact(&mut body).await.unwrap();
                let body = String::from_utf8(body).unwrap();
                assert!(!body.contains("private-peer"));
                let url = reqwest::Url::parse(&format!("http://fixture.invalid/?{body}")).unwrap();
                let fields: serde_json::Map<String, Value> = url
                    .query_pairs()
                    .map(|(key, value)| (key.into_owned(), json!(value)))
                    .collect();
                assert_eq!(fields["enc_ver"], "v2");
                let mut signed = fields.clone();
                let signature = signed.remove("signkey").unwrap();
                assert_eq!(signature, crate::crypto::sign_key("get", &signed));
                let params = url
                    .query_pairs()
                    .find(|(key, _)| key == "params")
                    .unwrap()
                    .1
                    .into_owned();
                assert!(!params.is_empty());
                let response = r#"{"error_code":114,"error_message":"private-server-text"}"#;
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).as_bytes()).await.unwrap();
            });
            let mut session = Session::new(false, false, false).unwrap();
            session.http = reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap();
            session.secret_key = Some(vec![7; 32]);
            session
                .service_resolver
                .write()
                .unwrap()
                .register_service("chat", vec![host.clone()]);
            assert!(session
                .resolve_service_url("chat", "/api/e2ee/pc/prekey/get")
                .starts_with(&host));
            let error = request(
                &session,
                "/api/e2ee/pc/prekey/get",
                &json!({"toid":"private-peer"}),
            )
            .await
            .unwrap_err();
            assert!(error.to_string().contains("114"));
            assert_eq!(
                serde_json::to_string(&classify(&error)).unwrap(),
                "\"other\""
            );
            server.await.unwrap();
        }
    }
}

#[cfg(feature = "experimental-e2ee")]
pub(crate) use experiment::send;

#[cfg(any(feature = "experimental-e2ee", test))]
fn reserve(conn: &rusqlite::Connection, profile: &str, peer: &str, id: u64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        "INSERT OR IGNORE INTO e2ee_trial_attempts(profile_id,peer_id,client_id) VALUES(?1,?2,?3)",
        rusqlite::params![profile, peer, id.to_string()],
    )?;
    anyhow::ensure!(
        changed == 1,
        "Lần gửi E2EE này đã được thử; kiểm tra hội thoại trước khi tạo tin nhắn mới"
    );
    tx.execute(
        "INSERT OR IGNORE INTO e2ee_required(profile_id,peer_id) VALUES(?1,?2)",
        rusqlite::params![profile, peer],
    )?;
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_contains_no_raw_identifiers_or_payloads() {
        let trace = Trace::new("private-account", "private-peer", Stage::Load, 37);
        let json = serde_json::to_string(&trace.event).unwrap();
        assert!(!json.contains("private-account"));
        assert!(!json.contains("private-peer"));
        assert!(!json.contains("message"));
        assert_eq!(correlation("a", "b"), correlation("a", "b"));
        assert_ne!(correlation("a", "b").2, correlation("c", "b").2);
    }
    #[test]
    fn crypto_failure_classification_does_not_serialize_error_text() {
        for (input, expected) in [
            (
                "E2EE MAC verification failed private-message",
                "authentication",
            ),
            ("replay attack private-message", "replay"),
            ("Chữ ký signedPreKey private-message", "signature"),
            ("private-message https://secret.invalid/token", "other"),
        ] {
            let output = serde_json::to_string(&classify(&anyhow::anyhow!(input))).unwrap();
            assert_eq!(output, format!("\"{expected}\""));
            assert!(!output.contains("private-message"));
        }
    }
    #[test]
    fn reservation_survives_reopen_and_is_account_scoped() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("crypto.db");
        let conn = crate::crypto::store::open_crypto_db_at(&path).unwrap();
        reserve(&conn, "a", "b", 123).unwrap();
        drop(conn);
        let conn = crate::crypto::store::open_crypto_db_at(&path).unwrap();
        assert!(reserve(&conn, "a", "b", 123).is_err());
        reserve(&conn, "c", "b", 123).unwrap();
    }
    #[test]
    fn schema_six_upgrade_preserves_identity_policy_and_adds_trial_reservations() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("legacy.db");
        let conn = crate::crypto::store::open_crypto_db_at(&path).unwrap();
        conn.execute("INSERT INTO e2ee_required VALUES('owner','peer')", [])
            .unwrap();
        conn.execute_batch(
            "DROP TABLE e2ee_trial_attempts; DROP TABLE e2ee_received; PRAGMA user_version=6;",
        )
        .unwrap();
        drop(conn);
        let conn = crate::crypto::store::open_crypto_db_at(&path).unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM e2ee_required WHERE profile_id='owner' AND peer_id='peer'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        reserve(&conn, "owner", "peer", 1).unwrap();
    }
}
