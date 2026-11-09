//! HTTP session: cookie jar, domain table, common params, request encoding.
//!
//! Mirrors the `_get` / `_post` / `_getCommonParams` / `getEncodedParams` helpers from
//! `compact-app-pc-*.js`. See `re-notes/02_chat_data.md` §2–§4.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{bail, Context, Result};
use reqwest::cookie::Jar;
use reqwest::header::{HeaderMap, HeaderValue, ORIGIN, REFERER};
use reqwest::{Client, Method};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::crypto;

pub const ZPW_VER: &str = "694";
pub const ZPW_TYPE: &str = "30";
pub const CONFIG_DOMAIN: &str = "zaloapp.com";

/// Domain table.
///
/// [O] **Live-verified** against a QR-authenticated Zalo Web session on 2026-10-21.
/// Every host below answered `error_code 0` for its endpoint. Note these are
/// `*.chat.zalo.me`, not the `*.zaloapp.com` names the PC bundle defaults to — the two
/// flavours differ, and using the wrong set yields nginx 404s.
///
/// [`Session::apply_server_info`] re-applies them from the login payload each run.
/// Only explicit authentication errors expire a saved login. Offline/HTTP/parse
/// failures must leave the protected session and local history intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Valid,
    Expired,
    Unreachable(String),
}

pub fn is_expired_error(message: &str) -> bool {
    message.split("server error ").skip(1).any(|part| {
        matches!(
            part.split(|c: char| !c.is_ascii_digit() && c != '-').next(),
            Some("600" | "-102")
        )
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domains {
    // Worker snapshots share endpoint preferences. This short lock never covers I/O.
    #[serde(skip)]
    winners: std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<String, String>>>,
    #[serde(default)]
    pub service_fallbacks: std::collections::BTreeMap<String, Vec<String>>,
    /// `wpa.zaloapp.com` — `reqqr`, password login.
    pub auth: String,
    /// `wpa.chat.zalo.me` — `getServerInfo` / `getLoginInfo` once a session exists.
    pub wpa_chat: String,
    /// `zpw_service_map_v3.chat[0]` — message send (`tt-chat*-wpa.*`).
    pub chat: String,
    /// `zpw_service_map_v3.conversation[0]` — conversation control (pins, preload).
    pub conversation: String,
    /// `zpw_chts_new[0]` — HTTP long-polling message receiver (`getChatFromPolling`, `p*-msg-chat.zaloapp.com`).
    #[serde(default = "default_lp_chat")]
    pub lp_chat: String,
    /// `zpw_ctrs_new[0]` — HTTP long-polling control receiver (`getControllFromPolling`, `p*-ctl-chat.zaloapp.com`).
    #[serde(default = "default_lp_control")]
    pub lp_control: String,
    /// `zpw_service_map_v3.friend[0]` — `social/friend/getfriends`.
    pub friend: String,
    /// `zpw_service_map_v3.conversation[0]` — `conv/*`, `hiddenconvers/*`.
    pub convers_api: String,
    /// `zpw_service_map_v3.profile[0]` — `getprofiles`, `me-v2`.
    pub profile: String,
    pub group: String,
    pub file: String,
    #[serde(default = "default_reaction")]
    pub reaction: String,
    #[serde(default)]
    pub group_board: String,
    /// GROUP_POLL is a separate service from reminders and notes.
    #[serde(default)]
    pub group_poll: String,
    /// `zpw_service_map_v3.group_cloud_message[0]` — `/api/cm/getoldv2`, `/api/cm/getrecentv2`.
    #[serde(default = "default_cloud_msg")]
    pub cloud_msg: String,
    /// `zpw_service_map_v3.voice_call[0]` — `/api/voicecall/*` signaling endpoints.
    #[serde(default = "default_voice_call")]
    pub voice_call: String,
    /// `zpw_service_map_v3.sticker[0]` — `/api/message/sticker/*` catalog & search endpoints.
    #[serde(default = "default_sticker")]
    pub sticker: String,
    /// `zpw_ws[...]` — control websocket endpoints (`wss://wsN-msg.chat.zalo.me`).
    ///
    /// [O] The server hands back full `wss://` URLs in the login config; the client never
    /// builds them from a literal. Real-time VoIP invites only arrive over this transport.
    #[serde(default = "default_ws")]
    pub ws: Vec<String>,
}

fn default_lp_chat() -> String {
    crate::endpoints::LP_CHAT_LEGACY.into()
}

fn default_lp_control() -> String {
    crate::endpoints::LP_CONTROL_LEGACY.into()
}

fn default_cloud_msg() -> String {
    crate::endpoints::CLOUD_MESSAGE_LEGACY.into()
}

fn default_voice_call() -> String {
    crate::endpoints::VOICE_CALL.into()
}

fn default_sticker() -> String {
    crate::endpoints::service("sticker")
        .unwrap_or(crate::endpoints::STICKER)
        .into()
}

/// [O] `re-notes/02_chat_data.md`: `zpw_ws` = `wss://ws{1..8}-msg.chat.zalo.me`.
fn default_ws() -> Vec<String> {
    crate::endpoints::default_websockets()
}

impl Default for Domains {
    fn default() -> Self {
        Self {
            winners: Default::default(),
            service_fallbacks: Default::default(),
            auth: crate::endpoints::AUTH.into(),
            wpa_chat: crate::endpoints::WPA_CHAT.into(),
            chat: crate::endpoints::service("chat").unwrap().into(),
            conversation: crate::endpoints::service("conversation").unwrap().into(),
            lp_chat: default_lp_chat(),
            lp_control: default_lp_control(),
            friend: crate::endpoints::FRIEND.into(),
            convers_api: crate::endpoints::CONVERSATION.into(),
            profile: crate::endpoints::PROFILE.into(),
            group: crate::endpoints::GROUP_LEGACY.into(),
            file: crate::endpoints::FILE_LEGACY.into(),
            reaction: default_reaction(),
            group_board: String::new(),
            group_poll: String::new(),
            cloud_msg: default_cloud_msg(),
            voice_call: default_voice_call(),
            sticker: default_sticker(),
            ws: default_ws(),
        }
    }
}

impl Domains {
    /// Keep verified REST hosts first; service discovery may advertise a PC host
    /// which returns 404 for a Web/QR session. Long-poll and WS stay dynamic.
    fn prefer_verified_rest(&mut self) {
        self.service_fallbacks.retain(|key, values| {
            let known = [
                "chat",
                "friend",
                "conversation",
                "profile",
                "group",
                "file",
                "group_cloud_message",
                "voice_call",
                "sticker",
                "reaction",
                "alias",
                "label",
            ]
            .iter()
            .filter_map(|key| crate::endpoints::service(key))
            .any(|host| host == key);
            if !known {
                return false;
            }
            let mut seen = std::collections::HashSet::new();
            values.retain(|host| trusted_service_url(host) && seen.insert(host.clone()));
            values.truncate(16);
            true
        });
        for (key, slot) in [
            ("chat", &mut self.chat),
            ("friend", &mut self.friend),
            ("conversation", &mut self.convers_api),
            ("profile", &mut self.profile),
            ("group", &mut self.group),
            ("file", &mut self.file),
            ("reaction", &mut self.reaction),
            ("voice_call", &mut self.voice_call),
            ("sticker", &mut self.sticker),
        ] {
            if let Some(verified) = crate::endpoints::service(key) {
                if slot != verified && trusted_service_url(slot) {
                    let fallback = self.service_fallbacks.entry(verified.into()).or_default();
                    if !fallback.contains(slot) {
                        fallback.push(slot.clone());
                    }
                }
                *slot = verified.into();
            }
        }
        self.conversation = self.convers_api.clone();
    }
}
fn trusted_service_url(value: &str) -> bool {
    reqwest::Url::parse(value)
        .map(|url| {
            url.scheme() == "https"
                && url.username().is_empty()
                && url.password().is_none()
                && url.port().is_none()
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none()
                && url
                    .host_str()
                    .is_some_and(|h| h.ends_with(".zalo.me") || h.ends_with(".zaloapp.com"))
        })
        .unwrap_or(false)
}
#[derive(Debug)]
struct HttpFailure(u16);
impl std::fmt::Display for HttpFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HTTP {}", self.0)
    }
}
impl std::error::Error for HttpFailure {}

/// Extract a typed HTTP failure without parsing or exposing its original error text.
pub(crate) fn http_failure_status(error: &anyhow::Error) -> Option<u16> {
    error.downcast_ref::<HttpFailure>().map(|failure| failure.0)
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct LoginInfo {
    pub uin: String,
    /// `zpw_enk` — base64 AES-256 key for legacy param encryption.
    pub zpw_enk: String,
    /// `zpw_sek` — session encryption/cookie token for authenticated endpoints.
    pub zpw_sek: String,
    pub display_name: String,
    pub send2me_id: String,
    #[serde(default)]
    pub avatar: String,
}

impl std::fmt::Debug for LoginInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginInfo")
            .field("uin", &self.uin)
            .field("zpw_enk", &"<redacted>")
            .field("zpw_sek", &"<redacted>")
            .field("display_name", &self.display_name)
            .field("send2me_id", &self.send2me_id)
            .field("avatar", &self.avatar)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SavedSession {
    pub imei: String,
    pub computer_name: String,
    pub secret_key_b64: Option<String>,
    pub session_key: Option<String>,
    pub zpw_sek: Option<String>,
    pub login: Option<LoginInfo>,
    pub domains: Domains,
}

impl std::fmt::Debug for SavedSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SavedSession")
            .field("imei", &crate::session::redact_secret(&self.imei))
            .field("computer_name", &self.computer_name)
            .field(
                "secret_key_b64",
                &self.secret_key_b64.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "session_key",
                &self.session_key.as_ref().map(|_| "<redacted>"),
            )
            .field("zpw_sek", &self.zpw_sek.as_ref().map(|_| "<redacted>"))
            .field("login", &self.login)
            .field("domains", &self.domains)
            .finish()
    }
}

#[derive(Clone)]
pub struct Session {
    pub service_resolver:
        std::sync::Arc<std::sync::RwLock<crate::service_resolver::ServiceResolver>>,
    runtime_generation: usize,
    /// Shared in-memory PIN gate, never serialized with login credentials.
    pub hidden: Arc<crate::hidden_chat::Vault>,
    pub inbox_policy: Arc<std::sync::RwLock<crate::inbox::Policy>>,
    /// Permission generations reject metadata fetched before an invalidating control.
    pub group_revision: Arc<std::sync::atomic::AtomicU64>,
    active: Arc<AtomicBool>,
    pub http: Client,
    pub jar: Arc<Jar>,
    pub domains: Domains,
    pub imei: String,
    pub computer_name: String,
    pub secret_key: Option<Vec<u8>>,
    /// Same key as a string, for `maybe_decrypt`.
    pub session_key: Option<String>,
    pub zpw_sek: Option<String>,
    pub login: Option<LoginInfo>,
    /// Draw commands in front of the user so they can be reproduced by hand.
    pub verbose: bool,
    /// Append every exchange (pretty-printed) to `dump/<n>-<slug>.json`.
    pub dump: bool,
    /// Skip URL redaction in dumps (needed to reproduce signature failures).
    pub dump_raw: bool,
}

// Pointer addresses can be recycled while permission-cache entries are still alive.
fn next_runtime_generation() -> usize {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Scrub sensitive fields from response body for diagnostic dumps (A14 fix).
///
/// Preserves structure and error codes for debugging, removes credentials and PII.
fn redact_body(body: &serde_json::Value) -> serde_json::Value {
    use serde_json::{json, Value};

    match body {
        Value::Object(map) => {
            let mut redacted = serde_json::Map::new();
            for (key, value) in map {
                let key_lower = key.to_lowercase();
                // Preserve error codes and status for debugging
                if key_lower.contains("error") || key_lower == "code" || key_lower == "status" {
                    redacted.insert(key.clone(), value.clone());
                }
                // Redact known sensitive fields
                else if key_lower.contains("token")
                    || key_lower.contains("password")
                    || key_lower.contains("secret")
                    || key_lower.contains("key")
                    || key_lower.contains("cookie")
                    || key_lower.contains("session")
                    || key_lower.contains("auth")
                    || key_lower.contains("credential")
                {
                    redacted.insert(key.clone(), json!("<REDACTED>"));
                }
                // Recursively scrub nested objects
                else if value.is_object() || value.is_array() {
                    redacted.insert(key.clone(), redact_body(value));
                }
                // Keep safe primitive values
                else {
                    redacted.insert(key.clone(), value.clone());
                }
            }
            Value::Object(redacted)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_body).collect()),
        _ => body.clone(),
    }
}
impl Session {
    pub fn new(verbose: bool, dump: bool, dump_raw: bool) -> Result<Self> {
        let jar = Arc::new(Jar::default());
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            REFERER,
            HeaderValue::from_static(crate::endpoints::WEB_ORIGIN),
        );
        default_headers.insert(ORIGIN, HeaderValue::from_static(crate::endpoints::WEB_BASE));
        let http = crate::proxy::apply(Client::builder())?
            .cookie_provider(Arc::clone(&jar))
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(default_headers)
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(35))
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) ZaloPC/26.10.11 Chrome/122.0.0.0 Electron/29.0.0 Safari/537.36",
            )
            .build()?;
        Ok(Self {
            service_resolver: std::sync::Arc::new(std::sync::RwLock::new(
                crate::service_resolver::ServiceResolver::new(),
            )),
            runtime_generation: next_runtime_generation(),
            hidden: Arc::new(crate::hidden_chat::Vault::default()),
            inbox_policy: Arc::new(std::sync::RwLock::new(crate::inbox::Policy::default())),
            group_revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            active: Arc::new(AtomicBool::new(true)),
            http,
            jar,
            domains: Domains::default(),
            imei: random_imei(),
            computer_name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "DESKTOP".into()),
            secret_key: None,
            session_key: None,
            zpw_sek: None,
            login: None,
            verbose,
            dump,
            dump_raw,
        })
    }

    /// Store `zpw_sek` session token, attach it to `self.login`, and insert the session cookie.
    pub fn set_zpw_sek(&mut self, v: String) {
        if v.is_empty() {
            return;
        }
        // Insert cookie for all required Zalo hosts
        let hosts = [
            crate::endpoints::WEB_BASE,
            crate::endpoints::AUTH,
            crate::endpoints::WPA_CHAT,
            crate::endpoints::STICKER,
            crate::endpoints::STICKER_LEGACY,
            crate::endpoints::STICKER_COMPAT,
            &self.domains.auth,
            &self.domains.chat,
            &self.domains.conversation,
            &self.domains.lp_chat,
            &self.domains.lp_control,
            &self.domains.convers_api,
            &self.domains.profile,
            &self.domains.friend,
            &self.domains.group,
            &self.domains.reaction,
            &self.domains.group_board,
            &self.domains.cloud_msg,
            &self.domains.voice_call,
            &self.domains.sticker,
        ];
        for host in hosts {
            if let Ok(url) = reqwest::Url::parse(host) {
                let cookie = format!("zpw_sek={v}; Path=/; Secure; HttpOnly");
                self.jar.add_cookie_str(&cookie, &url);
            }
        }
        self.zpw_sek = Some(v.clone());
        if let Some(ref mut li) = self.login {
            li.zpw_sek = v;
        }
    }

    /// Check the server without treating a transient network failure as logout.
    /// Resolve service URL via dynamic ServiceResolver with domain rotation.
    pub fn resolve_service_url(&self, service: &str, path: &str) -> String {
        self.service_resolver
            .read()
            .unwrap()
            .resolve_url(service, path)
    }

    pub fn record_service_failure(&self, service: &str) -> bool {
        self.service_resolver
            .write()
            .unwrap()
            .record_failure(service)
    }

    pub async fn validate_session(&self) -> SessionStatus {
        if self.login.is_none() || self.zpw_sek.is_none() {
            return SessionStatus::Expired;
        }
        let body = serde_json::json!({ "avatar_size": 160, "imei": self.imei });
        let enc = match self.encoded_params(&body) {
            Ok(enc) => enc,
            Err(e) => return SessionStatus::Unreachable(e.to_string()),
        };
        let url = format!(
            "{}/api/social/profile/me-v2?{}&params={enc}&os=8",
            self.domains.profile,
            self.common_params()
        );
        match self.get(&url).await {
            Ok(resp) => {
                let resp = self.maybe_decrypt(resp, self.session_key_bytes()).await;
                match resp.get("error_code").and_then(Value::as_i64) {
                    Some(0) => SessionStatus::Valid,
                    Some(600 | -102) => SessionStatus::Expired,
                    _ => SessionStatus::Unreachable("Không đọc được phản hồi xác thực".into()),
                }
            }
            Err(e) if is_expired_error(&e.to_string()) => SessionStatus::Expired,
            Err(e) => SessionStatus::Unreachable(e.to_string()),
        }
    }

    /// Fetch the authenticated user's own numeric `userId` and `displayName` from `/api/social/profile/me-v2`.
    pub async fn sync_profile_me(&mut self) -> Option<(String, String)> {
        if self.login.is_none() || self.zpw_sek.is_none() {
            return None;
        }
        let body = serde_json::json!({
            "avatar_size": 160,
            "imei": self.imei,
        });
        let enc = self.encoded_params(&body).ok()?;
        let url = format!(
            "{}/api/social/profile/me-v2?{}&params={enc}&os=8",
            self.domains.profile,
            self.common_params()
        );
        let resp = self.get(&url).await.ok()?;
        let dec = self.maybe_decrypt(resp, self.session_key_bytes()).await;
        let profile = dec
            .get("data")
            .and_then(|d| d.get("profile"))
            .or_else(|| dec.get("profile"))?;
        let uid = profile
            .get("userId")
            .or_else(|| profile.get("uid"))
            .and_then(|v| match v {
                Value::String(s) if !s.is_empty() => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })?;
        let name = profile
            .get("displayName")
            .or_else(|| profile.get("zaloName"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let avatar = profile
            .get("avatar")
            .or_else(|| profile.get("avt"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if let Some(ref mut li) = self.login {
            li.uin = uid.clone();
            if !name.is_empty() {
                li.display_name = name.clone();
            }
            if !avatar.is_empty() {
                li.avatar = avatar;
            }
        }
        Some((uid, name))
    }

    /// Clear all session credentials, tokens, keys, and cookies.
    pub fn logout(&mut self) {
        let _lock = crate::storage::SESSION_IO
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        self.active.store(false, Ordering::SeqCst);
        self.active = Arc::new(AtomicBool::new(true));
        self.runtime_generation = next_runtime_generation();
        self.login = None;
        self.hidden = Arc::new(crate::hidden_chat::Vault::default());
        self.inbox_policy = Arc::new(std::sync::RwLock::new(crate::inbox::Policy::default()));
        self.group_revision = Arc::new(std::sync::atomic::AtomicU64::new(0));
        self.zpw_sek = None;
        self.secret_key = None;
        self.session_key = None;
        self.jar = Arc::new(Jar::default());
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            REFERER,
            HeaderValue::from_static(crate::endpoints::WEB_ORIGIN),
        );
        default_headers.insert(ORIGIN, HeaderValue::from_static(crate::endpoints::WEB_BASE));
        if let Ok(c) = crate::proxy::apply(Client::builder()).and_then(|builder| {
            Ok(builder
            .cookie_provider(Arc::clone(&self.jar))
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(default_headers)
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(35))
            .user_agent(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) ZaloPC/26.10.11 Chrome/122.0.0.0 Electron/29.0.0 Safari/537.36",
            )
            .build()?)
        }) {
            self.http = c;
        }
    }

    /// Process-local identity shared by clones and replaced when login is revoked.
    pub(crate) fn runtime_id(&self) -> usize {
        self.runtime_generation
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Revoke clones before publishing an account switch; credential cleanup follows in logout.
    pub(crate) fn revoke_runtime(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    pub fn account_key(&self) -> &str {
        self.login
            .as_ref()
            .map(|l| {
                if !l.uin.is_empty() {
                    l.uin.as_str()
                } else {
                    l.send2me_id.as_str()
                }
            })
            .unwrap_or("")
    }

    pub fn history_path(&self) -> std::path::PathBuf {
        crate::storage::account_file(self.account_key(), "zalo_history.db")
    }

    pub fn cache_path(&self) -> std::path::PathBuf {
        crate::storage::account_file(self.account_key(), "threads_cache.json")
    }

    /// Save the authenticated session to disk so subsequent runs can restore it without re-authenticating.
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        use base64::Engine;
        let _lock = crate::storage::SESSION_IO
            .lock()
            .map_err(|_| anyhow::anyhow!("Session lock poisoned"))?;
        anyhow::ensure!(self.is_active(), "This session has been logged out");
        if !crate::storage::remember_login()? {
            crate::storage::remove_session_unlocked()?;
            return Ok(());
        }
        let saved = SavedSession {
            imei: self.imei.clone(),
            computer_name: self.computer_name.clone(),
            secret_key_b64: self
                .secret_key
                .as_ref()
                .map(|k| base64::engine::general_purpose::STANDARD.encode(k)),
            session_key: self.session_key.clone(),
            zpw_sek: self.zpw_sek.clone(),
            login: self.login.clone(),
            domains: self.domains.clone(),
        };
        let text = serde_json::to_string_pretty(&saved)?;
        let protected = crate::storage::protect(text.as_bytes())?;
        crate::storage::atomic_write(&crate::storage::session_path(path), &protected)?;
        drop(_lock);
        if path == "session.json" && !self.account_key().is_empty() {
            crate::profiles::remember_session(self)?;
        }
        Ok(())
    }

    /// Load a previously saved session from disk and restore cookies, keys, and endpoints.
    pub fn load_from_file(&mut self, path: &str) -> Result<()> {
        use base64::Engine;
        if !crate::storage::remember_login()? {
            crate::storage::remove_session()?;
            anyhow::bail!("Remember login is disabled");
        }
        if path == "session.json" && crate::storage::session_path(path).exists() {
            let registry = crate::profiles::ProfileRegistry::load()?;
            if let Some(profile) = registry.active() {
                anyhow::ensure!(
                    profile.session_file.is_absolute(),
                    "Đường dẫn phiên tài khoản không hợp lệ"
                );
                self.load_from_file(
                    profile
                        .session_file
                        .to_str()
                        .context("Đường dẫn phiên không hợp lệ")?,
                )?;
                anyhow::ensure!(
                    self.account_key() == profile.uid,
                    "Phiên đăng nhập không khớp tài khoản đã chọn"
                );
                return Ok(());
            }
        }
        let bytes = std::fs::read(crate::storage::session_path(path))?;
        let text = crate::storage::unprotect(&bytes)?;
        let saved: SavedSession = serde_json::from_slice(&text)?;

        self.imei = if saved.imei.len() == 69 {
            saved.imei
        } else {
            random_imei()
        };
        self.computer_name = saved.computer_name;
        self.domains = saved.domains;
        self.domains.prefer_verified_rest();
        // Auto-heal older session.json files where `zpw_chts_new` / `zpw_ctrs_new` overwrote REST domains
        if self.domains.chat.contains("-msg-chat.")
            || self.domains.chat.contains("tt-chat2-wpa")
            || self.domains.chat.is_empty()
        {
            if self.domains.chat.contains("-msg-chat.") && self.domains.lp_chat.is_empty() {
                self.domains.lp_chat = self.domains.chat.clone();
            }
            self.domains.chat = crate::endpoints::service("chat")
                .unwrap_or(crate::endpoints::CHAT_LEGACY)
                .into();
        }
        if self.domains.conversation.contains("-ctl-chat.") || self.domains.conversation.is_empty()
        {
            if self.domains.conversation.contains("-ctl-chat.")
                && self.domains.lp_control.is_empty()
            {
                self.domains.lp_control = self.domains.conversation.clone();
            }
            self.domains.conversation = crate::endpoints::service("conversation")
                .unwrap_or(crate::endpoints::CONVERSATION)
                .into();
        }
        if self.domains.sticker.is_empty() {
            self.domains.sticker = crate::endpoints::service("sticker")
                .unwrap_or(crate::endpoints::STICKER)
                .into();
        }
        self.session_key = saved.session_key;
        let sek = saved
            .zpw_sek
            .filter(|s| !s.is_empty())
            .or_else(|| saved.login.as_ref().map(|l| l.zpw_sek.clone()))
            .filter(|s| !s.is_empty());

        if let Some(b64) = saved.secret_key_b64 {
            if let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(b64) {
                self.secret_key = Some(raw);
            }
        }

        self.login = saved.login;

        if let Some(s) = sek {
            self.set_zpw_sek(s);
        } else {
            self.login = None;
            anyhow::bail!("session.json thiếu mã xác thực zpw_sek");
        }

        Ok(())
    }

    /// Persist one request/response pair for later schema mining.
    ///
    /// Credentials are redacted; the point is the response *shape*, not the content.
    fn dump_exchange(&self, method: &str, url: &str, status: u16, body: &str) {
        let dump_dir = crate::storage::root().join("diagnostics/dump");
        let dir = dump_dir.as_path();
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("! dump dir: {e}");
            return;
        }
        let n = std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0) + 1;
        // Name the file from the path only; the query carries ids we should not keep.
        let path_part = url.split('?').next().unwrap_or(url);
        let slug: String = path_part
            .split("/api/")
            .nth(1)
            .unwrap_or("root")
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let path = dir.join(format!("{n:02}-{slug}.json"));

        // Parse to a real JSON object so the dump nests properly instead of holding a
        // double-encoded string.
        let body_json =
            serde_json::from_str::<Value>(body).unwrap_or_else(|_| Value::String(body.to_string()));

        let doc = serde_json::json!({
            "method": method,
            // A14: Always scrub sensitive fields in diagnostic dumps, even in dev mode / dump_raw
            "url": redact_url(url),
            "status": status,
            "body": redact_body(&body_json),
        });
        let text = serde_json::to_string_pretty(&doc).unwrap_or_default();
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("! dump write: {e}");
        } else {
            eprintln!("  dumped → {}", path.display());
        }
    }

    /// `_constructUrlParams(se)` → `zpw_ver=693&zpw_type=30`. That is the whole object.
    pub fn common_params(&self) -> String {
        format!("zpw_ver={ZPW_VER}&zpw_type={ZPW_TYPE}")
    }

    /// The raw session AES key as bytes, for response decryption.
    pub fn session_key_bytes(&self) -> &[u8] {
        self.secret_key.as_deref().unwrap_or(&[])
    }

    /// `getEncodedParams(obj)` → url-encoded `encodeAES(json)`.
    ///
    /// Requires the session key; before login there is nothing to encrypt, so callers
    /// that run pre-auth use plain params instead.
    pub fn encoded_params(&self, v: &Value) -> Result<String> {
        let key = self
            .secret_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no session key yet (pre-login)"))?;
        let b64 = crypto::encode_aes(key, &v.to_string())?;
        Ok(urlencode(&b64))
    }

    /// Plain `params=<url-encoded json>` — accepted by the server when param encryption
    /// is off, which is the MVP path.
    pub fn plain_params(v: &Value) -> String {
        format!("params={}", urlencode(&v.to_string()))
    }

    /// Re-apply the endpoint table from a `getServerInfo` / `getLoginInfo` payload.
    ///
    /// [O] `setEndpoints(chts, ctrs, serviceMapNew, conf, ws)`:
    /// `zpw_chts_new` → chat, `zpw_ctrs_new` → conversation,
    /// `zpw_service_map_new.{friend,conversation,profile,group,file}` → the rest.
    pub fn apply_server_info(&mut self, data: &Value) {
        fn first(v: &Value) -> Option<String> {
            v.as_array()
                .and_then(|a| a.first())
                .and_then(|x| x.as_str())
                .map(|s| s.trim_end_matches('/').to_string())
                .or_else(|| v.as_str().map(|s| s.trim_end_matches('/').to_string()))
        }

        if let Some(v) = data.get("zpw_chts_new").and_then(first) {
            self.domains.lp_chat = v;
        }
        if let Some(v) = data.get("zpw_ctrs_new").and_then(first) {
            self.domains.lp_control = v;
        }
        if let Some(arr) = data.get("zpw_ws").and_then(|v| v.as_array()) {
            let list: Vec<String> = arr
                .iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.trim_end_matches('/').to_string())
                .filter(|s| s.starts_with("wss://") || s.starts_with("ws://"))
                .collect();
            if !list.is_empty() {
                self.domains.ws = list;
            }
        }
        if let Some(map) = data
            .get("zpw_service_map_v3")
            .or_else(|| data.get("zpw_service_map_new"))
        {
            for key in [
                "chat",
                "friend",
                "conversation",
                "profile",
                "group",
                "file",
                "group_cloud_message",
                "voice_call",
                "sticker",
                "reaction",
                "alias",
                "label",
            ] {
                if let Some(verified) = crate::endpoints::service(key) {
                    let candidates = self
                        .domains
                        .service_fallbacks
                        .entry(verified.into())
                        .or_default();
                    let values = map
                        .get(key)
                        .map(|v| v.as_array().cloned().unwrap_or_else(|| vec![v.clone()]))
                        .unwrap_or_default();
                    for value in values {
                        if let Some(host) = value.as_str().map(|s| s.trim_end_matches('/')) {
                            if trusted_service_url(host)
                                && host != verified
                                && !candidates.iter().any(|h| h == host)
                                && candidates.len() < 16
                            {
                                candidates.push(host.into());
                            }
                        }
                    }
                }
            }

            for (key, slot) in [
                ("chat", &mut self.domains.chat),
                ("friend", &mut self.domains.friend),
                ("conversation", &mut self.domains.convers_api),
                ("profile", &mut self.domains.profile),
                ("group", &mut self.domains.group),
                ("group_board", &mut self.domains.group_board),
                ("group_poll", &mut self.domains.group_poll),
                ("file", &mut self.domains.file),
                ("reaction", &mut self.domains.reaction),
                ("group_cloud_message", &mut self.domains.cloud_msg),
                ("voice_call", &mut self.domains.voice_call),
                ("sticker", &mut self.domains.sticker),
            ] {
                if let Some(v) = map.get(key).and_then(first) {
                    if trusted_service_url(&v) {
                        *slot = v;
                    }
                }
            }
            if let Some(v) = map.get("conversation").and_then(first) {
                self.domains.conversation = v;
            }
        }
        self.domains.prefer_verified_rest();
    }

    pub async fn get(&self, url: &str) -> Result<Value> {
        self.send(Method::GET, url, None).await
    }

    pub async fn post_form(&self, url: &str, body: String) -> Result<Value> {
        self.send(Method::POST, url, Some(body)).await
    }

    /// Send an RFC 7578 `multipart/form-data` binary upload (`chunkContent` or `fileContent`)
    /// to Zalo's file/photo servers.
    pub async fn post_multipart(
        &self,
        url: &str,
        field_name: &str,
        file_name: &str,
        mime_type: &str,
        file_bytes: &[u8],
    ) -> Result<Value> {
        anyhow::ensure!(self.is_active(), "This session has been logged out");
        let boundary = format!(
            "----WebKitFormBoundary{}",
            crate::param_cipher::random_hex(16)
        );
        let safe_name = file_name.replace(['"', '\r', '\n'], "_");
        let head = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{field_name}\"; filename=\"{safe_name}\"\r\nContent-Type: {mime_type}\r\n\r\n"
        );
        let tail = format!("\r\n--{boundary}--\r\n");
        let mut body = Vec::with_capacity(head.len() + file_bytes.len() + tail.len());
        body.extend_from_slice(head.as_bytes());
        body.extend_from_slice(file_bytes);
        body.extend_from_slice(tail.as_bytes());

        self.send_multipart_body(
            url,
            field_name,
            &safe_name,
            &boundary,
            reqwest::Body::from(body),
            file_bytes.len() as u64,
            None,
        )
        .await
    }

    pub(crate) async fn post_multipart_file(
        &self,
        url: &str,
        field_name: &str,
        file_name: &str,
        mime: &str,
        file: tokio::fs::File,
        size: u64,
    ) -> Result<Value> {
        use futures_util::StreamExt;
        let boundary = format!("----ctdozlite{}", crate::param_cipher::random_hex(16));
        let safe_name = file_name.replace(['"', '\r', '\n'], "_");
        let head = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{field_name}\"; filename=\"{safe_name}\"\r\nContent-Type: {mime}\r\n\r\n");
        let tail = format!("\r\n--{boundary}--\r\n");
        let length = size
            .checked_add(head.len() as u64)
            .and_then(|n| n.checked_add(tail.len() as u64))
            .ok_or_else(|| anyhow::anyhow!("Tệp quá lớn"))?;
        let first =
            futures_util::stream::once(
                async move { Ok::<Vec<u8>, std::io::Error>(head.into_bytes()) },
            );
        let chunks =
            futures_util::stream::try_unfold((file, size), |(mut file, remaining)| async move {
                use tokio::io::AsyncReadExt;
                if remaining == 0 {
                    return Ok(None);
                }
                let mut chunk = vec![0; remaining.min(crate::upload::CHUNK as u64) as usize];
                file.read_exact(&mut chunk).await?;
                Ok::<_, std::io::Error>(Some((
                    chunk,
                    (
                        file,
                        remaining - crate::upload::CHUNK.min(remaining as usize) as u64,
                    ),
                )))
            });
        let last =
            futures_util::stream::once(
                async move { Ok::<Vec<u8>, std::io::Error>(tail.into_bytes()) },
            );
        let body = reqwest::Body::wrap_stream(first.chain(chunks).chain(last));
        self.send_multipart_body(
            url,
            field_name,
            &safe_name,
            &boundary,
            body,
            size,
            Some(length),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_multipart_body(
        &self,
        url: &str,
        field_name: &str,
        safe_name: &str,
        boundary: &str,
        body: reqwest::Body,
        size: u64,
        length: Option<u64>,
    ) -> Result<Value> {
        anyhow::ensure!(self.is_active(), "This session has been logged out");
        let start = std::time::Instant::now();
        let mut rb = self
            .http
            .request(Method::POST, url)
            .timeout(std::time::Duration::from_secs(90))
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            );
        if let Some(sek) = &self.zpw_sek {
            if !sek.is_empty() && credential_target(url) {
                rb = rb
                    .header("Cookie", format!("zpw_sek={sek}; _zlang=vi"))
                    .header("zpw_sek", sek)
                    .header("Origin", crate::endpoints::WEB_BASE)
                    .header("Referer", crate::endpoints::WEB_ORIGIN);
            }
        }
        if let Some(length) = length {
            rb = rb.header("Content-Length", length);
        }
        let resp = rb.body(body).send().await?;
        let latency_ms = start.elapsed().as_millis();
        let status = resp.status();
        let bytes =
            crate::limits::response_bytes(resp, crate::limits::INFLATED_BYTES as usize).await?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        crate::logger::log_request(
            "POST(multipart)",
            url,
            Some(status.as_u16()),
            latency_ms,
            &format!("[{field_name}: {safe_name} ({} bytes)]", size),
            Some(&text),
            if status.is_success() {
                None
            } else {
                Some("HTTP multipart error")
            },
        );
        if !status.is_success() {
            return Err(HttpFailure(status.as_u16()).into());
        }
        if text.trim().is_empty() {
            return Ok(serde_json::json!({ "error_code": 0, "data": null }));
        }
        let v: Value = serde_json::from_str(&text)
            .unwrap_or_else(|_| serde_json::json!({ "data": text.trim() }));
        Ok(v)
    }

    async fn send(&self, m: Method, url: &str, body: Option<String>) -> Result<Value> {
        let mut urls = vec![url.to_string()];
        let mut service = None;
        for (verified, fallbacks) in &self.domains.service_fallbacks {
            if let Some(suffix) = url.strip_prefix(&format!("{verified}/")) {
                urls.extend(fallbacks.iter().map(|base| format!("{base}/{suffix}")));
                service = Some((verified.clone(), suffix.to_owned()));
                if let Some(winner) = self
                    .domains
                    .winners
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get(verified)
                    .filter(|w| *w == verified || fallbacks.contains(w))
                    .cloned()
                {
                    let preferred = format!("{winner}/{suffix}");
                    urls.retain(|u| u != &preferred);
                    urls.insert(0, preferred);
                }
                break;
            }
        }
        let mut last = None;
        for candidate in urls {
            match self.send_once(m.clone(), &candidate, body.clone()).await {
                Ok(response) => {
                    if let Some((service, suffix)) = &service {
                        if let Some(host) = candidate.strip_suffix(&format!("/{suffix}")) {
                            self.domains
                                .winners
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .insert(service.clone(), host.into());
                        }
                    }
                    return Ok(response);
                }
                Err(e) => {
                    // Retry GET on transport failure, or any method on a definite
                    // 404. Never replay a possibly delivered mutation after timeout.
                    let retry = e.downcast_ref::<HttpFailure>().is_some_and(|e| e.0 == 404)
                        || (m == Method::GET
                            && e.downcast_ref::<reqwest::Error>()
                                .is_some_and(|e| e.is_connect() || e.is_timeout()));
                    if !retry {
                        return Err(e);
                    }
                    last = Some(e);
                }
            }
        }
        Err(last.unwrap_or_else(|| anyhow::anyhow!("No service endpoint")))
    }

    async fn send_once(&self, m: Method, url: &str, body: Option<String>) -> Result<Value> {
        anyhow::ensure!(self.is_active(), "This session has been logged out");
        let start = std::time::Instant::now();
        if self.verbose {
            eprintln!("→ {} {}", m, redact_url(url));
            if let Some(b) = &body {
                eprintln!("  body: {}", crate::diagnostics::preview(b));
            }
        }
        let method = m.as_str().to_string();
        let mut rb = self.http.request(m, url);
        if let Some(ref sek) = self.zpw_sek {
            if !sek.is_empty() {
                let is_zalo_host = credential_target(url);
                if is_zalo_host {
                    rb = rb.header("Cookie", format!("zpw_sek={sek}; _zlang=vi"));
                    rb = rb.header("zpw_sek", sek);
                    rb = rb.header("Origin", crate::endpoints::WEB_BASE);
                    rb = rb.header("Referer", crate::endpoints::WEB_ORIGIN);
                }
            }
        }
        if let Some(b) = &body {
            rb = rb
                .header("Content-Type", "application/x-www-form-urlencoded")
                .body(b.clone());
        }
        let resp_result = rb.send().await;
        let latency_ms = start.elapsed().as_millis();

        let resp = match resp_result {
            Ok(r) => r,
            Err(e) => {
                crate::logger::log_request(
                    &method,
                    url,
                    None,
                    latency_ms,
                    body.as_deref().unwrap_or(""),
                    None,
                    Some(&e.to_string()),
                );
                return Err(e.without_url().into());
            }
        };

        let status = resp.status();
        let bytes =
            crate::limits::response_bytes(resp, crate::limits::INFLATED_BYTES as usize).await?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if self.verbose {
            eprintln!("← {status} response_bytes={}", text.len());
        }
        if self.dump {
            self.dump_exchange(&method, url, status.as_u16(), &text);
        }

        let is_err = !status.is_success();
        let err_str = if is_err {
            Some(format!("HTTP {status}"))
        } else {
            None
        };

        crate::logger::log_request(
            &method,
            url,
            Some(status.as_u16()),
            latency_ms,
            body.as_deref().unwrap_or(""),
            Some(&text),
            err_str.as_deref(),
        );

        if is_err {
            return Err(HttpFailure(status.as_u16()).into());
        }
        if text.trim().is_empty() {
            // Long-polling endpoints (`p4-msg-chat.zaloapp.com`) return HTTP 200 with an empty body `""`
            // on keepalive timeout (`if ("" === e.data) return void this.processEmptyData()`).
            return Ok(serde_json::json!({ "error_code": 0, "data": null }));
        }
        let v: Value = serde_json::from_str(&text).unwrap_or_else(|_| {
            // Encrypted responses arrive as a bare base64 blob, not JSON. Hand the raw
            // text back so callers can decrypt it instead of failing here.
            Value::String(text.clone())
        });
        let code = v.get("error_code").and_then(|c| c.as_i64()).unwrap_or(0);
        if code != 0 {
            let msg = v
                .get("error_message")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            crate::logger::log_request(
                &method,
                url,
                Some(status.as_u16()),
                latency_ms,
                body.as_deref().unwrap_or(""),
                Some(&text),
                Some(&format!("server error {code}: {msg}")),
            );
            bail!("server error {code}: {msg}");
        }
        Ok(v)
    }

    /// `decryptResp` — when the request was param-encrypted, `data` comes back as
    /// base64 AES over the key (AES-128 or AES-256), not plain JSON.
    pub async fn maybe_decrypt(&self, v: Value, key: &[u8]) -> Value {
        let Some(data) = v.get("data") else {
            return v;
        };
        let Some(b64) = data.as_str() else {
            return v;
        };
        if b64.is_empty() || b64.starts_with('{') {
            return v;
        }
        if key.is_empty() {
            return v;
        }
        match crate::crypto::decode_aes(key, b64) {
            Ok(pt) => match serde_json::from_str::<Value>(&pt) {
                Ok(inner) => {
                    if self.verbose {
                        eprintln!("  data decrypted ({} bytes)", pt.len());
                    }
                    // The decrypted payload is itself a full response envelope with its
                    // own error_code — propagate it instead of masking as success.
                    inner
                }
                Err(e) => {
                    if self.verbose {
                        eprintln!("  data decrypted but not JSON: {e}");
                    }
                    v
                }
            },
            Err(e) => {
                if self.verbose {
                    eprintln!("  data decrypt failed: {e}");
                }
                v
            }
        }
    }
}

/// Session headers are only sent over TLS to an actual Zalo host, never a
/// lookalike URL, plaintext media URL or an arbitrary multipart destination.
fn credential_target(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str().is_some_and(|h| {
                h == "zalo.me"
                    || h.ends_with(".zalo.me")
                    || h == "zaloapp.com"
                    || h.ends_with(".zaloapp.com")
            })
    })
}

/// Build `{a:1,b:"x"}` maps tersely.
pub fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

/// Strip anything that could be a credential before a URL lands in a dump file.
pub fn redact_url(url: &str) -> String {
    const SENSITIVE: &[&str] = &[
        "password", "passw", "zpw_sek", "zpw_enk", "cookie", "token", "signkey", "params", "imei",
        "secret", "auth", "session", "key",
    ];
    let Some((base, query)) = url.split_once('?') else {
        return url.to_string();
    };
    let cleaned: Vec<String> = query
        .split('&')
        .map(|kv| {
            let key = kv.split('=').next().unwrap_or("").to_ascii_lowercase();
            if SENSITIVE.iter().any(|s| key.contains(s)) {
                format!("{}=<redacted>", kv.split('=').next().unwrap_or(""))
            } else {
                kv.to_string()
            }
        })
        .collect();
    format!("{base}?{}", cleaned.join("&"))
}

/// Scrub sensitive tokens, secrets, cookies, passwords, and URLs from freeform text / error messages (A14).
pub fn redact_text(text: &str) -> String {
    if text.contains("http://") || text.contains("https://") {
        let mut result = String::with_capacity(text.len());
        let mut remaining = text;
        while let Some(idx) = remaining
            .find("http://")
            .or_else(|| remaining.find("https://"))
        {
            result.push_str(&remaining[..idx]);
            let url_part = &remaining[idx..];
            let end = url_part
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ')' || c == ']')
                .unwrap_or(url_part.len());
            let raw_url = &url_part[..end];
            result.push_str(&redact_url(raw_url));
            remaining = &url_part[end..];
        }
        result.push_str(remaining);
        return scrub_sensitive_tokens(&result);
    }
    scrub_sensitive_tokens(text)
}

fn scrub_sensitive_tokens(text: &str) -> String {
    let mut scrubbed = text.to_string();
    const PATTERNS: &[&str] = &[
        "zpw_sek",
        "zpw_enk",
        "token",
        "password",
        "passw",
        "secret_key",
        "session_key",
        "signkey",
        "cookie",
    ];
    for &pat in PATTERNS {
        for delim in &["=", ": ", ":"] {
            let needle = format!("{pat}{delim}");
            let needle_lower = needle.to_ascii_lowercase();
            let mut search_idx = 0;
            while let Some(pos) = scrubbed[search_idx..]
                .to_ascii_lowercase()
                .find(&needle_lower)
            {
                let abs_pos = search_idx + pos + needle.len();
                let val_end = scrubbed[abs_pos..]
                    .find(|c: char| {
                        c.is_whitespace()
                            || c == '&'
                            || c == ';'
                            || c == ','
                            || c == '}'
                            || c == ']'
                            || c == '"'
                    })
                    .unwrap_or(scrubbed.len() - abs_pos);
                if val_end > 0 {
                    scrubbed.replace_range(abs_pos..abs_pos + val_end, "<redacted>");
                }
                search_idx = abs_pos + "<redacted>".len();
                if search_idx >= scrubbed.len() {
                    break;
                }
            }
        }
    }
    scrubbed
}

/// Redact a sensitive secret string, showing at most the first 8 characters followed by `…<redacted>`.
pub fn redact_secret(s: &str) -> String {
    let prefix: String = s.chars().take(8).collect();
    format!("{prefix}…<redacted>")
}

pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let t: String = s.chars().take(n).collect();
        format!("{t}…")
    }
}

/// Client-generated device id. Persisted only in memory for now — regenerate per run.
fn random_imei() -> String {
    crate::call::official_imei()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_new_initializes_zpw_sek_and_headers() {
        let s = Session::new(false, false, false).unwrap();
        assert!(s.zpw_sek.is_none());
        assert!(s.login.is_none());
    }

    #[test]
    fn session_set_zpw_sek_updates_fields() {
        let mut s = Session::new(false, false, false).unwrap();
        s.login = Some(LoginInfo::default());
        s.set_zpw_sek("test_sek_value".into());
        assert_eq!(s.zpw_sek.as_deref(), Some("test_sek_value"));
        assert_eq!(s.login.as_ref().unwrap().zpw_sek, "test_sek_value");
    }

    #[test]
    fn redact_secret_truncates_to_eight_chars() {
        assert_eq!(redact_secret("1234567890abcdef"), "12345678…<redacted>");
        assert_eq!(redact_secret("short"), "short…<redacted>");
        assert_eq!(redact_secret(""), "…<redacted>");
    }

    #[test]
    fn redact_text_scrubs_sensitive_tokens_a14() {
        let err1 =
            "Request failed: https://chat.zalo.me/api/test?zpw_sek=SECRET123&token=TOK456 error";
        let scrubbed1 = redact_text(err1);
        assert!(!scrubbed1.contains("SECRET123"));
        assert!(!scrubbed1.contains("TOK456"));
        assert!(scrubbed1.contains("<redacted>"));

        let err2 = "Auth failed with zpw_sek=SECRET789 and password=MYPASS";
        let scrubbed2 = redact_text(err2);
        assert!(!scrubbed2.contains("SECRET789"));
        assert!(!scrubbed2.contains("MYPASS"));
        assert!(scrubbed2.contains("<redacted>"));
    }

    #[test]
    fn debug_impl_scrubs_sensitive_fields_a14() {
        let login = LoginInfo {
            uin: "12345".into(),
            zpw_enk: "ENK_SECRET".into(),
            zpw_sek: "SEK_SECRET".into(),
            display_name: "Test User".into(),
            send2me_id: "s2m".into(),
            avatar: "avatar.png".into(),
        };
        let dbg_login = format!("{login:?}");
        assert!(!dbg_login.contains("ENK_SECRET"));
        assert!(!dbg_login.contains("SEK_SECRET"));
        assert!(dbg_login.contains("<redacted>"));

        let saved = SavedSession {
            imei: "01234567890123456789".into(),
            computer_name: "PC".into(),
            secret_key_b64: Some("B64_KEY".into()),
            session_key: Some("SESS_KEY".into()),
            zpw_sek: Some("SEK_SAVED".into()),
            login: Some(login),
            domains: Domains::default(),
        };
        let dbg_saved = format!("{saved:?}");
        assert!(!dbg_saved.contains("B64_KEY"));
        assert!(!dbg_saved.contains("SESS_KEY"));
        assert!(!dbg_saved.contains("SEK_SAVED"));
        assert!(dbg_saved.contains("<redacted>"));
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;
    #[test]
    fn expiry_requires_exact_server_code() {
        for text in ["server error 600: expired", "server error -102: expired"] {
            assert!(is_expired_error(text));
        }
        for text in [
            "server error 6000",
            "HTTP 404",
            "zpw_sek missing in response",
            "timeout",
            "server error -1020",
        ] {
            assert!(!is_expired_error(text));
        }
    }
    #[test]
    fn discovery_preserves_verified_rest_and_dynamic_polling() {
        let mut s = Session::new(false, false, false).unwrap();
        let verified = s.domains.profile.clone();
        s.apply_server_info(&serde_json::json!({
            "zpw_service_map_v3": {"profile":["https://tt-profile-wpa.zaloapp.com"],"friend":["https://attacker.example"]},
            "zpw_chts_new":["https://p9-msg-chat.zaloapp.com"]
        }));
        assert_eq!(s.domains.profile, verified);
        assert_eq!(
            s.domains.service_fallbacks[&verified],
            vec!["https://tt-profile-wpa.zaloapp.com"]
        );
        assert_eq!(s.domains.lp_chat, "https://p9-msg-chat.zaloapp.com");
        assert!(!s
            .domains
            .service_fallbacks
            .values()
            .flatten()
            .any(|v| v.contains("attacker")));
        assert!(!s.domains.chat.contains("-msg-chat"));
    }
    #[tokio::test]
    async fn validation_distinguishes_expiry_from_server_and_parse_failures() {
        use std::io::{Read, Write};
        for (status, body, expected) in [
            (200, r#"{"error_code":0,"data":{}}"#, 0),
            (200, r#"{"error_code":600}"#, 1),
            (200, r#"{"error_code":-102}"#, 1),
            (404, "not found", 2),
            (503, "offline", 2),
            (200, "not json", 2),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request);
                write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
            });
            let mut s = Session::new(false, false, false).unwrap();
            s.http = reqwest::Client::builder().no_proxy().build().unwrap();
            s.domains.profile = format!("http://{addr}");
            s.login = Some(LoginInfo::default());
            s.secret_key = Some(vec![0; 32]);
            s.set_zpw_sek("fixture".into());
            let result = s.validate_session().await;
            match expected {
                0 => assert_eq!(result, SessionStatus::Valid),
                1 => assert_eq!(result, SessionStatus::Expired),
                _ => assert!(matches!(result, SessionStatus::Unreachable(_))),
            }
            assert_eq!(s.zpw_sek.as_deref(), Some("fixture"));
            server.join().unwrap();
        }
    }
}

#[cfg(test)]
mod credential_destination_tests {
    use super::*;
    #[test]
    fn only_secure_actual_zalo_hosts_receive_credentials() {
        assert!(credential_target("https://tt-profile-wpa.chat.zalo.me/api"));
        for url in [
            "http://tt-profile-wpa.chat.zalo.me/api",
            "https://zalo.me.evil.invalid/path",
            "https://zalo.me@evil.invalid/",
            "https://evil.invalid/?next=zalo.me",
            "invalid-zalo.me",
        ] {
            assert!(!credential_target(url), "{url}");
        }
    }
    #[tokio::test]
    async fn multipart_never_forwards_session_to_external_destination() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let header_end = loop {
                let mut buf = [0; 2048];
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buf[..n]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    break end + 4;
                }
            };
            let header = String::from_utf8_lossy(&request[..header_end]).to_string();
            let len: usize = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .map(|s| s.trim().parse().unwrap())
                })
                .unwrap();
            while request.len() < header_end + len {
                let mut buf = [0; 2048];
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                request.extend_from_slice(&buf[..n]);
            }
            assert!(!header.to_ascii_lowercase().contains("zpw_sek"));
            assert!(!header.to_ascii_lowercase().contains("cookie:"));
            let body = r#"{"error_code":0,"data":{}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        });
        let mut session = Session::new(false, false, false).unwrap();
        session.login = Some(LoginInfo::default());
        session.set_zpw_sek("fixture-token".into());
        session.http = reqwest::Client::builder()
            .no_proxy()
            .cookie_provider(session.jar.clone())
            .build()
            .unwrap();
        session
            .post_multipart(
                &format!("http://{address}/upload"),
                "fileContent",
                "fixture.bin",
                "application/octet-stream",
                b"fixture",
            )
            .await
            .unwrap();
        server.join().unwrap();
    }
}

#[cfg(test)]
mod candidate_persistence_tests {
    use super::*;
    #[test]
    fn label_and_alias_discovery_survive_sanitized_persistence() {
        let mut session = Session::new(false, false, false).unwrap();
        session.apply_server_info(&serde_json::json!({"zpw_service_map_v3":{"alias":["https://alias-fallback.zalo.me"],"label":["https://label-fallback.zaloapp.com"]}}));
        let mut restored: Domains =
            serde_json::from_value(serde_json::to_value(&session.domains).unwrap()).unwrap();
        restored.prefer_verified_rest();
        for (service, host) in [
            ("alias", "https://alias-fallback.zalo.me"),
            ("label", "https://label-fallback.zaloapp.com"),
        ] {
            assert!(
                restored.service_fallbacks[crate::endpoints::service(service).unwrap()]
                    .iter()
                    .any(|candidate| candidate == host)
            );
        }
    }
    #[test]
    fn discovery_retains_all_trusted_candidates_and_saved_candidates_are_sanitized() {
        let mut s = Session::new(false, false, false).unwrap();
        s.apply_server_info(&serde_json::json!({"zpw_service_map_v3":{"chat":["https://fallback-one.zalo.me","https://fallback-two.zaloapp.com","https://attacker.invalid"]}}));
        let primary = crate::endpoints::service("chat").unwrap();
        assert_eq!(s.domains.service_fallbacks[primary].len(), 2);
        let mut restored: Domains =
            serde_json::from_value(serde_json::to_value(&s.domains).unwrap()).unwrap();
        restored
            .service_fallbacks
            .get_mut(primary)
            .unwrap()
            .push("https://attacker.invalid".into());
        restored.prefer_verified_rest();
        assert_eq!(restored.service_fallbacks[primary].len(), 2);
        assert!(restored.winners.lock().unwrap().is_empty());
    }
}

#[cfg(test)]
mod remembered_host_tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn serve(status: &'static str) -> (String, tokio::task::JoinHandle<usize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let mut count = 0;
            while let Ok(Ok((mut stream, _))) =
                tokio::time::timeout(Duration::from_millis(800), listener.accept()).await
            {
                let mut bytes = Vec::new();
                loop {
                    let mut buffer = [0; 1024];
                    let n = stream.read(&mut buffer).await.unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
                let body = "{\"error_code\":0,\"data\":{}}";
                stream.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                count += 1;
            }
            count
        });
        (url, task)
    }
    #[tokio::test]
    async fn next_request_starts_at_the_host_that_previously_succeeded() {
        let (primary, first) = serve("404 Not Found").await;
        let (fallback, second) = serve("200 OK").await;
        let mut s = Session::new(false, false, false).unwrap();
        s.http = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        s.domains
            .service_fallbacks
            .insert(primary.clone(), vec![fallback.clone()]);
        for _ in 0..2 {
            s.get(&format!("{primary}/api/fixture")).await.unwrap();
        }
        assert_eq!(
            s.domains.winners.lock().unwrap().get(&primary),
            Some(&fallback)
        );
        assert_eq!(first.await.unwrap(), 1);
        assert_eq!(second.await.unwrap(), 2);
    }
}

fn default_reaction() -> String {
    crate::endpoints::service("reaction")
        .unwrap_or(crate::endpoints::MSG_CHAT)
        .into()
}

#[cfg(test)]
mod runtime_generation_tests {
    use super::*;
    #[test]
    fn revoked_runtimes_cannot_reuse_permission_cache_identity() {
        let mut session = Session::new(false, false, false).unwrap();
        let clone = session.clone();
        let original = session.runtime_id();
        assert_eq!(clone.runtime_id(), original);
        let other = Session::new(false, false, false).unwrap();
        assert_ne!(other.runtime_id(), original);
        session.logout();
        assert_ne!(session.runtime_id(), original);
        assert_eq!(clone.runtime_id(), original);
        assert!(!clone.is_active());
        assert!(!session.hidden.view("old").ready);
    }
}
