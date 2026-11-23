//! Pre-login flow: server info → password login → session bootstrap.
//!
//! Notes: `re-notes/01_login.md` §5.

use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::param_cipher::ParamCipher;
use crate::session::{obj, urlencode, Session, ZPW_TYPE, ZPW_VER};

/// Thread-safe auth attempt generation tracker to prevent race conditions
/// between concurrent or sequential login attempts (e.g. Password login vs QR login) (A19).
#[derive(Debug, Clone, Default)]
pub struct AuthAttemptManager {
    generation: Arc<AtomicU64>,
}

impl AuthAttemptManager {
    pub fn new() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Advance generation counter and return the new generation ID.
    pub fn next_attempt(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Current generation ID.
    pub fn current(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Check whether a response from a specific attempt generation is still valid/current.
    pub fn is_current(&self, attempt_id: u64) -> bool {
        self.current() == attempt_id
    }
}

/// PasswordCipher (class N) — `re-notes/01_login.md` §2.2.
///
/// `enk = even(sha256_upper("{pwKey}-{phone}-{imei}-{salt}"))[0..16]
///        + reverse(odd(sha256_upper(...)))[0..16]`
/// `encryptPassword = base64(AES256CBC(enk, iv=0, salt + reverse(pwd)))`
pub fn encrypt_password(phone: &str, imei: &str, password: &str, salt: &str) -> Result<String> {
    const PW_KEY: &str = "9fc5vEW3";

    let mut h = Sha256::new();
    h.update(format!("{PW_KEY}-{phone}-{imei}-{salt}").as_bytes());
    let hex = h.finalize().iter().fold(String::new(), |mut acc, b| {
        use std::fmt::Write;
        let _ = write!(acc, "{b:02X}");
        acc
    });

    let even: Vec<char> = hex.chars().step_by(2).collect();
    let odd: Vec<char> = hex.chars().skip(1).step_by(2).collect();
    let enk: String =
        even[..16].iter().collect::<String>() + &odd.iter().rev().take(16).collect::<String>();

    let mut plain = salt.to_string();
    plain.extend(password.chars().rev());

    crate::crypto::encode_aes(enk.as_bytes(), &plain)
}

const SALT_ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
const SALT_LEN: usize = 8;

/// Salt generator for encrypt_password: 8 alphanumerics from the OS CSPRNG.
/// Length and alphabet are unchanged; an RNG failure aborts login instead of
/// falling back to a predictable seed.
pub fn random_salt() -> Result<String> {
    salt_from(crate::random::fill)
}

/// Rejection sampling keeps every alphabet symbol equally likely (252 = 7 * 36).
fn salt_from(mut fill: impl FnMut(&mut [u8]) -> Result<()>) -> Result<String> {
    const LIMIT: u8 = (u8::MAX as usize / SALT_ALPHABET.len() * SALT_ALPHABET.len()) as u8;
    let mut out = String::with_capacity(SALT_LEN);
    let mut buf = [0u8; 16];
    while out.len() < SALT_LEN {
        fill(&mut buf)?;
        for byte in buf {
            if byte < LIMIT && out.len() < SALT_LEN {
                out.push(SALT_ALPHABET[usize::from(byte) % SALT_ALPHABET.len()] as char);
            }
        }
    }
    Ok(out)
}

/// `GET {auth}/api/login/getServerInfo` — unauthenticated, gates param encryption.
///
/// [O] Bundle body:
/// ```text
/// o = { imei, type: apiType, client_version: apiVersion, computer_name,
///       sp_mtn: -1, bkt: -1 }
/// o.signkey = getSignKey("getserverinfo", o)
/// GET {auth}/api/login/getServerInfo?a=1&b=2…   // params inline, NOT params=
/// ```
pub async fn get_server_info(s: &Session) -> Result<Value> {
    let mut p = serde_json::Map::new();
    p.insert("imei".into(), json!(s.imei));
    p.insert("type".into(), json!(ZPW_TYPE));
    p.insert("client_version".into(), json!(ZPW_VER));
    p.insert("computer_name".into(), json!(s.computer_name));
    p.insert("sp_mtn".into(), json!(-1));
    p.insert("bkt".into(), json!(-1));

    let sig = crate::crypto::sign_key("getserverinfo", &p);
    let query: String = p
        .iter()
        .map(|(k, v)| {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{}={}", k, urlencode(&val))
        })
        .collect::<Vec<_>>()
        .join("&");

    let url = format!(
        "{}/api/login/getServerInfo?{query}&signkey={sig}",
        s.domains.auth
    );
    s.get(&url).await
}

/// v1 password login: plain params, signkey over the params object itself.
///
/// [O] `signInPassword`: `o = {phone_num, password, cptch_token, cptch_answer, type,
/// client_version, computer_name, imei}`; then `o.ref`, then `o.signkey =
/// getSignKey("loginbypassw", o)`; POST `params=<encodeURIComponent(JSON)>`.
///
/// Kept as a diagnostic path — it exposes what the server objects to without the v2
/// encryption layer in the way.
pub async fn login_by_password_v1(s: &mut Session, phone: &str, password: &str) -> Result<Value> {
    let mut o = serde_json::Map::new();
    o.insert("phone_num".into(), json!(phone));
    o.insert("password".into(), json!(password));
    o.insert("cptch_token".into(), json!(""));
    o.insert("cptch_answer".into(), json!(""));
    o.insert("type".into(), json!(ZPW_TYPE));
    o.insert("client_version".into(), json!(ZPW_VER));
    o.insert("computer_name".into(), json!(s.computer_name));
    o.insert("imei".into(), json!(s.imei));
    o.insert("ref".into(), json!(crate::endpoints::WEB_BASE));

    let sig = crate::crypto::sign_key("loginbypassw", &o);
    o.insert("signkey".into(), json!(sig));

    let url = format!("{}/api/login/loginByPassw", s.domains.auth);
    let form = format!("params={}", urlencode(&Value::Object(o).to_string()));
    let resp = s.post_form(&url, form).await?;

    let code = resp.get("error_code").and_then(|c| c.as_i64()).unwrap_or(0);
    if code != 0 {
        let msg = resp
            .get("error_message")
            .and_then(|m| m.as_str())
            .unwrap_or("");
        anyhow::bail!("v1 login error {code}: {msg}");
    }
    if let Some(cfg) = resp.get("data") {
        apply_login_config(s, cfg);
    }
    Ok(resp)
}

/// Build the `signInPasswordV2` payload (`g` in compact-app-pc bundle).
///
/// When `encrypt_param == true`:
///   - Generates an 8-character CSPRNG `salt`.
///   - Encrypts password: `pwd = encrypt_password(phone, imei, password, &salt)`.
///   - Includes `"password": json!(pwd)` and `"salt": json!(salt)`.
///
/// When `encrypt_param == false`:
///   - Uses raw password: `"password": json!(password)`.
///   - Strictly omits `"salt"` (parity with Zalo PC `signInPasswordV2` E2).
pub fn build_login_payload(
    phone: &str,
    password: &str,
    iso: &str,
    imei: &str,
    computer_name: &str,
    encrypt_param: bool,
) -> Result<Value> {
    build_login_payload_inner(
        phone,
        password,
        iso,
        imei,
        computer_name,
        encrypt_param,
        None,
    )
}

pub(crate) fn build_login_payload_inner(
    phone: &str,
    password: &str,
    iso: &str,
    imei: &str,
    computer_name: &str,
    encrypt_param: bool,
    explicit_salt: Option<&str>,
) -> Result<Value> {
    if encrypt_param {
        let salt = match explicit_salt {
            Some(s) => s.to_string(),
            None => random_salt()?,
        };
        let pwd = encrypt_password(phone, imei, password, &salt)?;
        Ok(obj(&[
            ("phone_num", json!(phone)),
            ("iso_country_code", json!(iso)),
            ("password", json!(pwd)),
            ("salt", json!(salt)),
            ("cptch_token", json!("")),
            ("cptch_answer", json!("")),
            ("make_answer", json!(0)),
            ("language", json!("en")),
            ("logged_uids", json!(null)),
            ("computer_name", json!(computer_name)),
            ("imei", json!(imei)),
        ]))
    } else {
        Ok(obj(&[
            ("phone_num", json!(phone)),
            ("iso_country_code", json!(iso)),
            ("password", json!(password)),
            ("cptch_token", json!("")),
            ("cptch_answer", json!("")),
            ("make_answer", json!(0)),
            ("language", json!("en")),
            ("logged_uids", json!(null)),
            ("computer_name", json!(computer_name)),
            ("imei", json!(imei)),
        ]))
    }
}

/// Build request body for `loginByPasswv2` using the given cipher policy.
pub fn build_login_body(
    phone: &str,
    password: &str,
    iso: &str,
    imei: &str,
    computer_name: &str,
    encrypt_param: bool,
    cipher: &ParamCipher,
) -> Result<Value> {
    let payload = build_login_payload(phone, password, iso, imei, computer_name, encrypt_param)?;
    cipher.build_body_with_policy(ZPW_TYPE, ZPW_VER, &payload, "loginbypasswv2", encrypt_param)
}

/// `POST {auth}/api/login/loginByPasswv2` — ParamCipher v2 body.
///
/// [O] Exact bundle flow (`signInPasswordV2`):
/// ```text
/// g = {phone_num, iso_country_code, password, cptch_token, cptch_answer, make_answer,
///      language, logged_uids, computer_name, imei}
/// if (enable_encrypt_param) { n = new N({phone, imei}); g.salt = n.salt; g.password = n.encryptPassword(t) }
/// l && g.quest_cert = l;  c && g.cptch_cert = c;  h && g.last_uid = h;  m && g.viewerkey = m
/// {params, enk} = encryptParam(g, "loginbypasswv2")
/// le && (f.ref = window.location.origin)      // ← AFTER signing, NOT part of signkey
/// POST {auth}/api/login/loginByPasswv2?_constructUrlParams(f)
/// ```
/// Key detail: `salt` replaces the password only; `ref` is added post-signature.
pub async fn login_by_password(
    s: &mut Session,
    phone: &str,
    password: &str,
    iso: &str,
) -> Result<Value> {
    let client_time = unix_now();
    let mut encrypt_param = s.encrypt_param_enabled();
    let mut retried = false;

    loop {
        let cipher = ParamCipher::new(ZPW_TYPE, &s.imei, client_time)?;
        let body = build_login_body(
            phone,
            password,
            iso,
            &s.imei,
            &s.computer_name,
            encrypt_param,
            &cipher,
        )?;

        // Signed set only — no `enk`, no `ref` (both are outside the hash server-side).
        let query = flatten_query(&body);
        let url = format!(
            "{}/api/login/loginByPasswv2?{query}&ref={}",
            s.domains.auth,
            urlencode(crate::endpoints::WEB_BASE)
        );

        let resp = s.post_form(&url, String::new()).await?;
        let resp = s.maybe_decrypt(resp, cipher.key().as_bytes()).await;

        // Surface an error code hidden inside the encrypted envelope.
        let code = resp.get("error_code").and_then(|c| c.as_i64()).unwrap_or(0);
        if code == 19000 && encrypt_param && !retried {
            s.handle_api_error_code(19000);
            encrypt_param = false;
            retried = true;
            continue;
        }

        if code != 0 {
            let msg = resp
                .get("error_message")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            anyhow::bail!("login error {code}: {msg}");
        }

        if let Some(cfg) = resp.get("data") {
            apply_login_config(s, cfg);
        }
        return Ok(resp);
    }
}

/// `_constructUrlParams(obj)` — flatten a JSON object into `k=v&k=v`.
fn flatten_query(v: &Value) -> String {
    let Some(m) = v.as_object() else {
        return String::new();
    };
    m.iter()
        .map(|(k, val)| {
            let sv = match val {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("{}={}", k, urlencode(&sv))
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// `GET /api/login/getLoginInfo` — returns session keys (dkey, zpw_enk, endpoints).
pub async fn get_login_info(s: &Session) -> Result<Value> {
    let client_time = unix_now();
    let payload = obj(&[
        ("imei", json!(s.imei)),
        ("computer_name", json!(s.computer_name)),
        ("language", json!("vi")),
        ("ts", json!(client_time)),
    ]);

    let mut encrypt_param = s.encrypt_param_enabled();
    let mut retried = false;

    loop {
        let cipher = ParamCipher::new(ZPW_TYPE, &s.imei, client_time)?;
        let body = cipher.build_body_with_policy(
            ZPW_TYPE,
            ZPW_VER,
            &payload,
            "getlogininfo",
            encrypt_param,
        )?;
        let query = flatten_query(&body);

        // Try both auth host (wpa.zaloapp.com, where PC session cookies live)
        // and wpa_chat (wpa.chat.zalo.me, where web session cookies live).
        let hosts = [&s.domains.auth, &s.domains.wpa_chat];
        let mut last_err = anyhow::anyhow!("get_login_info failed on all hosts");
        let mut saw_19000 = false;

        for host in hosts {
            let url = format!("{host}/api/login/getLoginInfo?{query}&nretry=0");
            match s.get(&url).await {
                Ok(resp) => {
                    let decrypted = s.maybe_decrypt(resp, cipher.key().as_bytes()).await;
                    let code = decrypted
                        .get("error_code")
                        .and_then(|c| c.as_i64())
                        .unwrap_or(0);
                    if code == 0 {
                        return Ok(decrypted);
                    }
                    if code == 19000 {
                        s.handle_api_error_code(19000);
                        saw_19000 = true;
                        break;
                    }
                }
                Err(e) => {
                    last_err = e;
                }
            }
        }

        if saw_19000 && encrypt_param && !retried {
            encrypt_param = false;
            retried = true;
            continue;
        }

        return Err(last_err);
    }
}

/// Pull `dkey` / `zpw_enk` / domains out of a login response payload.
///
/// Pull `dkey` / `zpw_enk` / domains out of a login response payload.
/// Preserves already established session credentials if the new payload only contains partial fields.
pub fn apply_login_config(s: &mut Session, cfg: &Value) {
    if let Some(config) = crate::composer_config::ComposerConfig::from_login_config(cfg) {
        s.composer_config = Some(config);
    }
    let new_dkey = cfg
        .get("uid")
        .or_else(|| cfg.get("userId"))
        .or_else(|| {
            cfg.get("account")
                .and_then(|a| a.get("userId").or_else(|| a.get("uid")))
        })
        .or_else(|| cfg.get("dkey"))
        .and_then(as_str)
        .unwrap_or_default();

    let new_enk = cfg.get("zpw_enk").and_then(as_str).unwrap_or_default();
    let new_send2me_id = cfg.get("send2me_id").and_then(as_str).unwrap_or_default();

    let new_sek = cfg
        .get("zpw_sek")
        .or_else(|| cfg.get("account").and_then(|a| a.get("zpw_sek")))
        .and_then(as_str)
        .unwrap_or_default();

    let new_name = cfg
        .get("account")
        .and_then(|a| a.get("display_name"))
        .or_else(|| cfg.get("display_name"))
        .or_else(|| {
            cfg.get("settings")
                .and_then(|st| st.get("profile"))
                .and_then(|p| p.get("name"))
        })
        .and_then(as_str)
        .unwrap_or_default();

    if !new_enk.is_empty() {
        let raw = crate::crypto::parse_secret_key(&new_enk);
        s.secret_key = Some(raw);
    }
    if !new_sek.is_empty() {
        s.set_zpw_sek(new_sek.clone());
    }

    let mut current = s.login.take().unwrap_or_default();
    if !new_dkey.is_empty() {
        current.uin = new_dkey.to_string();
    }
    if !new_enk.is_empty() {
        current.zpw_enk = new_enk.to_string();
    }
    if !new_sek.is_empty() {
        current.zpw_sek = new_sek.to_string();
    } else if current.zpw_sek.is_empty() {
        if let Some(ref sek) = s.zpw_sek {
            current.zpw_sek = sek.clone();
        }
    }
    if !new_name.is_empty() {
        current.display_name = new_name.to_string();
    }
    if !new_send2me_id.is_empty() {
        current.send2me_id = new_send2me_id.to_string();
    }

    s.login = Some(current);

    // Domain overrides: `zpw_chts_new` and `zpw_ctrs_new` are long-polling hosts,
    // while `zpw_service_map_v3` holds the REST API domains.
    s.apply_server_info(cfg);
    if let Some(v) = cfg.get("zpw_chts_new").and_then(as_str) {
        if !v.is_empty() {
            s.domains.lp_chat = normalize_host(&v);
        }
    }
    if let Some(v) = cfg.get("zpw_ctrs_new").and_then(as_str) {
        if !v.is_empty() {
            s.domains.lp_control = normalize_host(&v);
        }
    }
}

fn normalize_host(v: &str) -> String {
    if v.starts_with("http") {
        v.trim_end_matches('/').to_string()
    } else {
        format!("https://{}", v.trim_end_matches('/'))
    }
}

fn as_str(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salt_uses_injected_rng_with_exact_length_and_alphabet() {
        // Bytes >= 252 are rejected; 0, 35 and 251 map to 'a', '9' and '9'.
        let script = [255u8, 252, 0, 35, 251, 36, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut calls = 0;
        let salt = salt_from(|buf| {
            calls += 1;
            buf.copy_from_slice(&script);
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!(salt, "a99abcde");
        assert_eq!(salt.len(), SALT_LEN);
        assert!(salt.bytes().all(|b| SALT_ALPHABET.contains(&b)));
    }

    #[test]
    fn salt_requests_more_entropy_when_bytes_are_rejected() {
        let mut calls = 0;
        let salt = salt_from(|buf| {
            calls += 1;
            buf.fill(if calls == 1 { 254 } else { 1 });
            Ok(())
        })
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(salt, "bbbbbbbb");
    }

    #[test]
    fn salt_propagates_rng_failure_without_fallback() {
        let error = salt_from(|_| anyhow::bail!("rng down")).unwrap_err();
        assert!(error.to_string().contains("rng down"));
    }

    #[test]
    fn production_salt_comes_from_os_rng() {
        let salt = random_salt().unwrap();
        assert_eq!(salt.len(), SALT_LEN);
        assert!(salt.bytes().all(|b| SALT_ALPHABET.contains(&b)));
    }

    #[test]
    fn apply_login_config_captures_zpw_sek() {
        let mut s = Session::new(false, false, false).unwrap();
        let cfg = serde_json::json!({
            "dkey": "12345",
            "zpw_enk": "dGVzdF9lbmtfMzJfYnl0ZXNfbG9uZ19hZXNfa2V5IQ==",
            "zpw_sek": "sek_abc_123",
            "display_name": "Test User"
        });
        apply_login_config(&mut s, &cfg);
        assert_eq!(s.zpw_sek.as_deref(), Some("sek_abc_123"));
        let login = s.login.as_ref().unwrap();
        assert_eq!(login.zpw_sek, "sek_abc_123");
        assert_eq!(login.uin, "12345");
    }

    #[test]
    fn apply_login_config_captures_nested_account_zpw_sek() {
        let mut s = Session::new(false, false, false).unwrap();
        let cfg = serde_json::json!({
            "dkey": "12345",
            "zpw_enk": "dGVzdF9lbmtfMzJfYnl0ZXNfbG9uZ19hZXNfa2V5IQ==",
            "account": {
                "zpw_sek": "sek_nested_456",
                "display_name": "Nested User"
            }
        });
        apply_login_config(&mut s, &cfg);
        assert_eq!(s.zpw_sek.as_deref(), Some("sek_nested_456"));
        let login = s.login.as_ref().unwrap();
        assert_eq!(login.zpw_sek, "sek_nested_456");
    }

    #[test]
    fn test_login_attempt_generation_monotonic_and_race_prevention() {
        let manager = AuthAttemptManager::new();
        assert_eq!(manager.current(), 0);

        let attempt_1 = manager.next_attempt();
        assert_eq!(attempt_1, 1);
        assert!(manager.is_current(attempt_1));

        let attempt_2 = manager.next_attempt();
        assert_eq!(attempt_2, 2);
        assert!(manager.is_current(attempt_2));
        // Stale attempt 1 must be rejected!
        assert!(!manager.is_current(attempt_1));

        // Attempt 3 started (e.g. user requested QR after password submit)
        let attempt_3 = manager.next_attempt();
        assert_eq!(attempt_3, 3);
        assert!(!manager.is_current(attempt_1));
        assert!(!manager.is_current(attempt_2));
        assert!(manager.is_current(attempt_3));
    }

    #[test]
    fn test_build_login_payload_encrypted_branch() {
        let phone = "0912345678";
        let password = "super_secret_password";
        let iso = "VN";
        let imei = "fake-imei-123";
        let computer_name = "test-pc";

        let payload = build_login_payload(phone, password, iso, imei, computer_name, true).unwrap();
        assert_eq!(payload["phone_num"], phone);
        assert_eq!(payload["iso_country_code"], iso);
        assert_eq!(payload["cptch_token"], "");
        assert_eq!(payload["cptch_answer"], "");
        assert_eq!(payload["make_answer"], 0);
        assert_eq!(payload["language"], "en");
        assert!(payload["logged_uids"].is_null());
        assert_eq!(payload["computer_name"], computer_name);
        assert_eq!(payload["imei"], imei);

        // Salt must be present, exactly 8 characters
        let salt = payload
            .get("salt")
            .and_then(Value::as_str)
            .expect("salt must be present");
        assert_eq!(salt.len(), 8);

        // Password must be encrypted (not raw password)
        let pwd = payload
            .get("password")
            .and_then(Value::as_str)
            .expect("password must be present");
        assert_ne!(pwd, password);

        // Verify password matches encrypt_password with this salt
        let expected_pwd = encrypt_password(phone, imei, password, salt).unwrap();
        assert_eq!(pwd, expected_pwd);
    }

    #[test]
    fn test_build_login_payload_unencrypted_branch_omits_salt() {
        let phone = "0912345678";
        let password = "super_secret_password";
        let iso = "VN";
        let imei = "fake-imei-123";
        let computer_name = "test-pc";

        let payload =
            build_login_payload(phone, password, iso, imei, computer_name, false).unwrap();
        assert_eq!(payload["phone_num"], phone);
        assert_eq!(payload["iso_country_code"], iso);
        assert_eq!(payload["cptch_token"], "");
        assert_eq!(payload["cptch_answer"], "");
        assert_eq!(payload["make_answer"], 0);
        assert_eq!(payload["language"], "en");
        assert!(payload["logged_uids"].is_null());
        assert_eq!(payload["computer_name"], computer_name);
        assert_eq!(payload["imei"], imei);

        // Crucial invariant: salt MUST NOT be present
        assert!(
            payload.get("salt").is_none(),
            "salt must be omitted when encrypt_param is false"
        );

        // Password must be the raw password
        assert_eq!(payload["password"], password);
    }

    #[test]
    fn test_build_login_body_encrypted_and_unencrypted_and_fallback_simulation() {
        let phone = "0912345678";
        let password = "super_secret_password";
        let iso = "VN";
        let imei = "fake-imei-123";
        let computer_name = "test-pc";
        let client_time = 1700000000;

        let cipher = ParamCipher::new(ZPW_TYPE, imei, client_time).unwrap();

        // 1. Encrypted branch: body contains "params" envelope and signkey
        let body_enc =
            build_login_body(phone, password, iso, imei, computer_name, true, &cipher).unwrap();
        assert!(body_enc.get("params").is_some());
        assert!(body_enc.get("signkey").is_some());
        assert_eq!(body_enc["type"], ZPW_TYPE);
        assert_eq!(body_enc["client_version"], ZPW_VER);
        // Envelope does not leak raw fields at top-level
        assert!(body_enc.get("phone_num").is_none());
        assert!(body_enc.get("password").is_none());
        assert!(body_enc.get("salt").is_none());

        // 2. Unencrypted branch: body has raw fields, raw password, NO salt, plus signkey
        let body_unenc =
            build_login_body(phone, password, iso, imei, computer_name, false, &cipher).unwrap();
        assert!(body_unenc.get("params").is_none());
        assert_eq!(body_unenc["phone_num"], phone);
        assert_eq!(body_unenc["password"], password);
        assert!(
            body_unenc.get("salt").is_none(),
            "salt must not appear in unencrypted body"
        );
        assert_eq!(body_unenc["type"], ZPW_TYPE);
        assert_eq!(body_unenc["client_version"], ZPW_VER);
        assert!(body_unenc.get("signkey").is_some());

        // 3. Fallback progression: simulating 19000
        let session = Session::new(false, false, false).unwrap();
        assert!(session.encrypt_param_enabled());

        let mut encrypt_param = session.encrypt_param_enabled();
        let payload_1 = build_login_payload(
            phone,
            password,
            iso,
            &session.imei,
            &session.computer_name,
            encrypt_param,
        )
        .unwrap();
        assert!(payload_1.get("salt").is_some());
        assert_ne!(payload_1["password"], password);

        // Server returns 19000
        let simulated_error_code = 19000;
        session.handle_api_error_code(simulated_error_code);
        assert!(!session.encrypt_param_enabled());

        encrypt_param = session.encrypt_param_enabled();
        let payload_2 = build_login_payload(
            phone,
            password,
            iso,
            &session.imei,
            &session.computer_name,
            encrypt_param,
        )
        .unwrap();
        assert!(payload_2.get("salt").is_none());
        assert_eq!(payload_2["password"], password);
    }
}
