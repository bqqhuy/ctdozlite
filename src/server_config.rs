//! Typed remote configuration lifecycle covering all 8 Zalo PC server config groups.
//!
//! Evidence: E2 static reads in `re-notes/17-zalo-pc-sync-config-and-reconnect.md`,
//! `re-notes/70-composer-remote-config.md`, `re-notes/72-restricted-extensions-and-native-executable-zip.md`,
//! and `docs/research/2026-11-22-direct-pc-web-android-analysis.md`.
//! Handles both `settings` and Zalo PC's 3-`t` `setttings` envelope spellings.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::Value;

use crate::composer_config::ComposerConfig;

pub const DEFAULT_RESTRICTED_EXTENSIONS: [&str; 6] = ["exe", "cmd", "bat", "com", "lnk", "vbs"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEecxConfig {
    pub enable_file_eecx: bool,
    pub restricted_ext_file: Vec<String>,
}

impl Default for FileEecxConfig {
    fn default() -> Self {
        Self {
            enable_file_eecx: true,
            restricted_ext_file: DEFAULT_RESTRICTED_EXTENSIONS
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
        }
    }
}

impl FileEecxConfig {
    pub fn is_restricted_extension(&self, ext: &str) -> bool {
        let normalized = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        !normalized.is_empty()
            && normalized != "zip"
            && self.restricted_ext_file.iter().any(|e| e == &normalized)
    }

    pub fn should_wrap_eecx(&self, ext: &str) -> bool {
        self.enable_file_eecx && self.is_restricted_extension(ext)
    }

    /// Evaluate filename policy safely including multi-dot names (`app.tar.gz.exe`),
    /// case-insensitive extensions, whitespace, and double-wrap prevention for `.zip`.
    pub fn should_wrap_filename(&self, name: &str) -> bool {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return false;
        }
        let Some(ext) = std::path::Path::new(trimmed)
            .extension()
            .and_then(|e| e.to_str())
        else {
            return false;
        };
        self.should_wrap_eecx(ext)
    }

    /// Evaluate filesystem path policy safely against the active EECX configuration.
    pub fn should_wrap_path(&self, path: &std::path::Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| self.should_wrap_filename(name))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DownloadDomainsSwapConfig {
    pub enabled: bool,
    pub domains_map: BTreeMap<String, String>,
    pub mode: u32,
}

impl DownloadDomainsSwapConfig {
    /// Rewrite hostname according to `domains_map` when enabled while preserving
    /// path, query, and signed URL parameters. Rejects unsafe/non-Zalo targets.
    pub fn rewrite_url(&self, raw: &str) -> String {
        if !self.enabled || self.domains_map.is_empty() {
            return raw.to_owned();
        }
        let Ok(mut url) = reqwest::Url::parse(raw) else {
            return raw.to_owned();
        };
        let Some(host) = url.host_str().map(|h| h.to_ascii_lowercase()) else {
            return raw.to_owned();
        };
        let Some(replacement) = self.domains_map.get(&host) else {
            return raw.to_owned();
        };
        if url.set_host(Some(replacement)).is_err() {
            return raw.to_owned();
        }
        if !crate::media_download::allowed(&url) {
            return raw.to_owned();
        }
        url.into()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sync2PcConfig {
    pub enable_auto_request_sync_on_log_in_new_device: bool,
    pub sync_suggestion_zalopc: bool,
    pub max_concurrent_sync_items: usize,
    pub batch_timeout_ms: u64,
    pub wakeup_timeout_ms: u64,
    pub awaiting_mobile_timeout_ms: u64,
    pub max_percent_batch_fail: f64,
}

impl Default for Sync2PcConfig {
    fn default() -> Self {
        Self {
            enable_auto_request_sync_on_log_in_new_device: false,
            sync_suggestion_zalopc: false,
            max_concurrent_sync_items: 4,
            batch_timeout_ms: 10_000,
            wakeup_timeout_ms: 180_000,
            awaiting_mobile_timeout_ms: 600_000,
            max_percent_batch_fail: 0.5,
        }
    }
}

impl Sync2PcConfig {
    pub fn awaiting_mobile_timeout(&self) -> Duration {
        Duration::from_millis(self.awaiting_mobile_timeout_ms)
    }

    pub fn wakeup_timeout(&self) -> Duration {
        Duration::from_millis(self.wakeup_timeout_ms)
    }

    pub fn batch_timeout(&self) -> Duration {
        Duration::from_millis(self.batch_timeout_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeepLoadingDuringPreloadConfig {
    pub enable: bool,
    pub timeout_duration_ms: u64,
    pub screen_dismiss_delay_ms: u64,
}

impl Default for KeepLoadingDuringPreloadConfig {
    fn default() -> Self {
        Self {
            enable: false,
            timeout_duration_ms: 800,
            screen_dismiss_delay_ms: 200,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct O2oConfig {
    pub keep_loading_during_preload: KeepLoadingDuringPreloadConfig,
    pub get_last_message_timeout_before_polling_ms: u64,
}

impl Default for O2oConfig {
    fn default() -> Self {
        Self {
            keep_loading_during_preload: KeepLoadingDuringPreloadConfig::default(),
            get_last_message_timeout_before_polling_ms: 10_000,
        }
    }
}

impl O2oConfig {
    pub fn keep_loading_timeout(&self) -> Duration {
        Duration::from_millis(self.keep_loading_during_preload.timeout_duration_ms)
    }

    pub fn screen_dismiss_delay(&self) -> Duration {
        Duration::from_millis(self.keep_loading_during_preload.screen_dismiss_delay_ms)
    }

    pub fn get_last_message_timeout(&self) -> Duration {
        Duration::from_millis(self.get_last_message_timeout_before_polling_ms)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerConfig {
    pub zpw_service_map_v3: BTreeMap<String, Vec<String>>,
    pub enable_encrypt_param: bool,
    pub zcinput: ComposerConfig,
    pub enable_feature_alias_v2: bool,
    pub file_eecx: FileEecxConfig,
    pub download_domains_swap: DownloadDomainsSwapConfig,
    pub sync_2_pc: Sync2PcConfig,
    pub o2o: O2oConfig,
    pub revision: u64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            zpw_service_map_v3: BTreeMap::new(),
            enable_encrypt_param: true,
            zcinput: ComposerConfig::default(),
            enable_feature_alias_v2: false,
            file_eecx: FileEecxConfig::default(),
            download_domains_swap: DownloadDomainsSwapConfig::default(),
            sync_2_pc: Sync2PcConfig::default(),
            o2o: O2oConfig::default(),
            revision: 0,
        }
    }
}

impl ServerConfig {
    /// Parse and merge validated configuration fields from either a full `{error_code, data}`
    /// envelope or an unwrapped `data` / `config` object.
    /// Returns `true` if the payload was structurally valid and applied.
    pub fn apply_payload(&mut self, payload: &Value) -> bool {
        if let Some(code) = payload.get("error_code") {
            if crate::api_response::integer(code) != Some(0) {
                return false;
            }
        }
        let data = payload.get("data").unwrap_or(payload);
        if !data.is_object() {
            return false;
        }

        // 1. zpw_service_map_v3 (fallback zpw_service_map_new)
        if let Some(map) = data
            .get("zpw_service_map_v3")
            .or_else(|| data.get("zpw_service_map_new"))
            .and_then(Value::as_object)
        {
            for (service, val) in map {
                let candidates = val.as_array().cloned().unwrap_or_else(|| vec![val.clone()]);
                let valid: Vec<String> = candidates
                    .into_iter()
                    .filter_map(|v| v.as_str().map(|s| s.trim_end_matches('/').to_owned()))
                    .filter(|host| crate::session::trusted_service_url_str(host))
                    .take(16)
                    .collect();
                if !valid.is_empty() {
                    self.zpw_service_map_v3.insert(service.clone(), valid);
                }
            }
        }

        // Collect feature containers (`setttings` 3-t spelling, `settings`, `config.settings`).
        let mut feature_roots: Vec<&Value> = Vec::new();
        for root in [Some(data), data.get("config")].into_iter().flatten() {
            for key in ["setttings", "settings"] {
                if let Some(st) = root.get(key) {
                    if let Some(features) = st.get("features") {
                        feature_roots.push(features);
                    }
                    feature_roots.push(st);
                }
            }
            if let Some(features) = root.get("features") {
                feature_roots.push(features);
            }
            feature_roots.push(root);
        }

        // 2. enable_encrypt_param
        if let Some(flag) = find_bool(&feature_roots, &["enable_encrypt_param"]) {
            self.enable_encrypt_param = flag;
        }

        // 3. zcinput
        if let Some(cfg) = ComposerConfig::from_server_info(payload)
            .or_else(|| ComposerConfig::from_login_config(data))
            .or_else(|| {
                feature_roots
                    .iter()
                    .find_map(|r| r.get("zcinput").and_then(ComposerConfig::from_value))
            })
        {
            self.zcinput = cfg;
        }

        // 4. enable_feature_alias_v2
        if let Some(flag) = find_bool(&feature_roots, &["enable_feature_alias_v2"])
            .or_else(|| find_nested_bool(&feature_roots, "alias", &["enable_feature_alias_v2"]))
        {
            self.enable_feature_alias_v2 = flag;
        }

        // 5. file_eecx & restricted_ext_file
        if let Some(flag) = find_bool(&feature_roots, &["enable_file_eecx"])
            .or_else(|| find_nested_bool(&feature_roots, "file", &["enable_file_eecx"]))
            .or_else(|| find_nested_bool(&feature_roots, "sharefile", &["enable_file_eecx"]))
        {
            self.file_eecx.enable_file_eecx = flag;
        }
        if let Some(exts) = find_value(&feature_roots, &["sharefile", "restricted_ext_file"])
            .or_else(|| find_value(&feature_roots, &["file", "restricted_ext_file"]))
            .or_else(|| find_value(&feature_roots, &["restricted_ext_file"]))
            .and_then(parse_extension_list)
        {
            let mut merged: Vec<String> = DEFAULT_RESTRICTED_EXTENSIONS
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            for ext in exts {
                if !merged.contains(&ext) && merged.len() < 64 {
                    merged.push(ext);
                }
            }
            self.file_eecx.restricted_ext_file = merged;
        }

        // 6. download_domains_swap
        if let Some(swap_val) = find_value(&feature_roots, &["download_domains_swap"]) {
            if let Some(obj) = swap_val.as_object() {
                if let Some(enabled) = obj.get("enabled").and_then(parse_bool) {
                    self.download_domains_swap.enabled = enabled;
                }
                if let Some(mode) = obj
                    .get("mode")
                    .and_then(Value::as_u64)
                    .and_then(|v| u32::try_from(v).ok())
                {
                    self.download_domains_swap.mode = mode;
                }
                if let Some(map_obj) = obj.get("domains_map").and_then(Value::as_object) {
                    let mut validated = BTreeMap::new();
                    for (src, dst) in map_obj {
                        let Some(dst_str) = dst.as_str() else {
                            continue;
                        };
                        if valid_domain_swap_entry(src, dst_str) && validated.len() < 64 {
                            validated
                                .insert(src.to_ascii_lowercase(), dst_str.to_ascii_lowercase());
                        }
                    }
                    self.download_domains_swap.domains_map = validated;
                }
            }
        }

        // 7. sync_2_pc
        if let Some(sync_val) = find_value(&feature_roots, &["sync_2_pc"])
            .or_else(|| find_value(&feature_roots, &["sync2pc"]))
        {
            apply_sync_2_pc(&mut self.sync_2_pc, sync_val);
        } else {
            // Also check flat keys under feature roots when server places flags directly.
            for root in &feature_roots {
                if root
                    .get("enable_auto_request_sync_on_log_in_new_device")
                    .is_some()
                    || root.get("sync_suggestion_zalopc").is_some()
                {
                    apply_sync_2_pc(&mut self.sync_2_pc, root);
                    break;
                }
            }
        }

        // 8. o2o
        if let Some(o2o_val) = find_value(&feature_roots, &["o2o"]) {
            apply_o2o(&mut self.o2o, o2o_val);
        }

        self.revision = self.revision.saturating_add(1);
        true
    }
}

fn parse_bool(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| crate::api_response::integer(value).map(|n| n != 0))
}

fn parse_u64_bounded(value: Option<&Value>, min: u64, max: u64) -> Option<u64> {
    let raw = value.and_then(|v| {
        v.as_u64()
            .or_else(|| v.as_i64().filter(|n| *n >= 0).map(|n| n as u64))
    })?;
    Some(raw.clamp(min, max))
}

fn find_bool(roots: &[&Value], keys: &[&str]) -> Option<bool> {
    for root in roots {
        for key in keys {
            if let Some(b) = root.get(*key).and_then(parse_bool) {
                return Some(b);
            }
        }
    }
    None
}

fn find_nested_bool(roots: &[&Value], parent: &str, keys: &[&str]) -> Option<bool> {
    for root in roots {
        if let Some(sub) = root.get(parent) {
            for key in keys {
                if let Some(b) = sub.get(*key).and_then(parse_bool) {
                    return Some(b);
                }
            }
        }
    }
    None
}

fn find_value<'a>(roots: &[&'a Value], path: &[&str]) -> Option<&'a Value> {
    for root in roots {
        let mut cur = *root;
        let mut matched = true;
        for seg in path {
            if let Some(next) = cur.get(*seg) {
                cur = next;
            } else {
                matched = false;
                break;
            }
        }
        if matched {
            return Some(cur);
        }
    }
    None
}

fn parse_extension_list(value: &Value) -> Option<Vec<String>> {
    let raw_items: Vec<&str> = match value {
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        Value::String(csv) => csv.split(',').collect(),
        _ => return None,
    };
    let mut parsed = Vec::new();
    for item in raw_items {
        let clean = item.trim().trim_start_matches('.').to_ascii_lowercase();
        if !clean.is_empty()
            && clean.len() <= 16
            && clean.bytes().all(|b| b.is_ascii_alphanumeric())
            && !parsed.contains(&clean)
        {
            parsed.push(clean);
        }
    }
    (!parsed.is_empty()).then_some(parsed)
}

fn valid_hostname(host: &str) -> bool {
    !host.is_empty()
        && host.len() <= 253
        && !host.contains('/')
        && !host.contains(':')
        && !host.contains('@')
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
}

fn valid_domain_swap_entry(src: &str, dst: &str) -> bool {
    if !valid_hostname(src) || !valid_hostname(dst) {
        return false;
    }
    let Ok(test_url) = reqwest::Url::parse(&format!("https://{dst}/")) else {
        return false;
    };
    crate::media_download::allowed(&test_url)
}

fn apply_sync_2_pc(cfg: &mut Sync2PcConfig, val: &Value) {
    if let Some(b) = val
        .get("enable_auto_request_sync_on_log_in_new_device")
        .and_then(parse_bool)
    {
        cfg.enable_auto_request_sync_on_log_in_new_device = b;
    }
    if let Some(b) = val.get("sync_suggestion_zalopc").and_then(parse_bool) {
        cfg.sync_suggestion_zalopc = b;
    }
    if let Some(n) = parse_u64_bounded(val.get("max_number_of_concurrent_sync_items"), 1, 16) {
        cfg.max_concurrent_sync_items = n as usize;
    }
    if let Some(n) = parse_u64_bounded(val.get("batch_timeout"), 1_000, 120_000) {
        cfg.batch_timeout_ms = n;
    }
    if let Some(n) = parse_u64_bounded(val.get("wakeup_timeout"), 5_000, 600_000) {
        cfg.wakeup_timeout_ms = n;
    }
    if let Some(n) = parse_u64_bounded(val.get("awaiting_mobile_timeout_ms"), 10_000, 1_800_000) {
        cfg.awaiting_mobile_timeout_ms = n;
    }
    if let Some(f) = val
        .get("max_percent_batch_fail")
        .and_then(Value::as_f64)
        .filter(|f| f.is_finite() && (0.0..=1.0).contains(f))
    {
        cfg.max_percent_batch_fail = f;
    }
}

fn apply_o2o(cfg: &mut O2oConfig, val: &Value) {
    if let Some(keep) = val.get("keep_loading_during_preload") {
        if let Some(b) = keep.get("enable").and_then(parse_bool) {
            cfg.keep_loading_during_preload.enable = b;
        }
        if let Some(ms) = parse_u64_bounded(
            keep.get("timeout_duration")
                .or_else(|| keep.get("timeout_duration_ms")),
            100,
            15_000,
        ) {
            cfg.keep_loading_during_preload.timeout_duration_ms = ms;
        }
        if let Some(ms) = parse_u64_bounded(
            keep.get("screen_dismiss_delay")
                .or_else(|| keep.get("screen_dismiss_delay_ms")),
            0,
            5_000,
        ) {
            cfg.keep_loading_during_preload.screen_dismiss_delay_ms = ms;
        }
    }
    if let Some(ms) = parse_u64_bounded(
        val.get("get_last_message_timeout_before_polling")
            .or_else(|| val.get("get_last_message_timeout_before_polling_ms")),
        1_000,
        60_000,
    ) {
        cfg.get_last_message_timeout_before_polling_ms = ms;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_all_eight_groups_with_three_t_setttings_and_resets_cleanly() {
        let payload = json!({
            "error_code": 0,
            "data": {
                "zpw_service_map_v3": {
                    "chat": ["https://chat-cfg.zalo.me", "https://evil.invalid"]
                },
                "setttings": {
                    "features": {
                        "enable_encrypt_param": 0,
                        "enable_feature_alias_v2": true,
                        "zcinput": {
                            "version": 1,
                            "behavior": { "inputMaxLines": 8.0 },
                            "style": { "lineHeight": 24.0 }
                        },
                        "sharefile": {
                            "enable_file_eecx": true,
                            "restricted_ext_file": ["msi", ".ps1", "../bad"]
                        },
                        "download_domains_swap": {
                            "enabled": true,
                            "mode": 2,
                            "domains_map": {
                                "f1.zdn.vn": "f2.zdn.vn",
                                "ok.zdn.vn": "attacker.invalid"
                            }
                        },
                        "sync_2_pc": {
                            "enable_auto_request_sync_on_log_in_new_device": true,
                            "sync_suggestion_zalopc": true,
                            "max_number_of_concurrent_sync_items": 6,
                            "batch_timeout": 15000,
                            "wakeup_timeout": 90000,
                            "awaiting_mobile_timeout_ms": 300000,
                            "max_percent_batch_fail": 0.25
                        },
                        "o2o": {
                            "keep_loading_during_preload": {
                                "enable": true,
                                "timeout_duration": 1200,
                                "screen_dismiss_delay": 350
                            },
                            "get_last_message_timeout_before_polling": 12000
                        }
                    }
                }
            }
        });

        let mut cfg = ServerConfig::default();
        assert!(cfg.apply_payload(&payload));
        assert_eq!(
            cfg.zpw_service_map_v3.get("chat"),
            Some(&vec!["https://chat-cfg.zalo.me".to_owned()])
        );
        assert!(!cfg.enable_encrypt_param);
        assert_eq!(cfg.zcinput.rich_max_lines, 8.0);
        assert_eq!(cfg.zcinput.line_height, 24.0);
        assert!(cfg.enable_feature_alias_v2);
        assert!(cfg.file_eecx.should_wrap_eecx("exe"));
        assert!(cfg.file_eecx.should_wrap_eecx("msi"));
        assert!(cfg.file_eecx.should_wrap_eecx("ps1"));
        assert!(!cfg.file_eecx.should_wrap_eecx("bad"));
        assert!(!cfg.file_eecx.should_wrap_eecx("pdf"));

        assert!(cfg.download_domains_swap.enabled);
        assert_eq!(cfg.download_domains_swap.mode, 2);
        assert_eq!(
            cfg.download_domains_swap
                .rewrite_url("https://f1.zdn.vn/a/b.png?sig=123"),
            "https://f2.zdn.vn/a/b.png?sig=123"
        );
        // Untrusted swap target is rejected and never stored.
        assert!(!cfg
            .download_domains_swap
            .domains_map
            .contains_key("ok.zdn.vn"));
        assert_eq!(
            cfg.download_domains_swap
                .rewrite_url("https://ok.zdn.vn/x.png?sig=1"),
            "https://ok.zdn.vn/x.png?sig=1"
        );

        assert!(cfg.sync_2_pc.enable_auto_request_sync_on_log_in_new_device);
        assert!(cfg.sync_2_pc.sync_suggestion_zalopc);
        assert_eq!(cfg.sync_2_pc.max_concurrent_sync_items, 6);
        assert_eq!(cfg.sync_2_pc.batch_timeout_ms, 15_000);
        assert_eq!(cfg.sync_2_pc.wakeup_timeout_ms, 90_000);
        assert_eq!(cfg.sync_2_pc.awaiting_mobile_timeout_ms, 300_000);
        assert_eq!(cfg.sync_2_pc.max_percent_batch_fail, 0.25);

        assert!(cfg.o2o.keep_loading_during_preload.enable);
        assert_eq!(
            cfg.o2o.keep_loading_during_preload.timeout_duration_ms,
            1200
        );
        assert_eq!(
            cfg.o2o.keep_loading_during_preload.screen_dismiss_delay_ms,
            350
        );
        assert_eq!(cfg.o2o.get_last_message_timeout_before_polling_ms, 12_000);
        assert_eq!(cfg.revision, 1);
    }

    #[test]
    fn error_envelope_does_not_apply_and_bounds_clamp_extreme_values() {
        let mut cfg = ServerConfig::default();
        assert!(!cfg
            .apply_payload(&json!({"error_code": -1, "data": {"enable_feature_alias_v2": true}})));
        assert!(!cfg.enable_feature_alias_v2);
        assert_eq!(cfg.revision, 0);

        assert!(cfg.apply_payload(&json!({
            "error_code": 0,
            "data": {
                "settings": {
                    "features": {
                        "o2o": {
                            "keep_loading_during_preload": {
                                "enable": 1,
                                "timeout_duration": 999999,
                                "screen_dismiss_delay": 999999
                            }
                        }
                    }
                }
            }
        })));
        assert!(cfg.o2o.keep_loading_during_preload.enable);
        assert_eq!(
            cfg.o2o.keep_loading_during_preload.timeout_duration_ms,
            15_000
        );
        assert_eq!(
            cfg.o2o.keep_loading_during_preload.screen_dismiss_delay_ms,
            5_000
        );
    }

    #[test]
    fn eecx_policy_handles_enable_flag_defaults_remote_merge_case_multidot_and_never_double_wraps_zip(
    ) {
        let mut cfg = ServerConfig::default();
        // Default 6 restricted extensions; MSI is NOT in default list.
        assert!(cfg.file_eecx.should_wrap_filename("setup.EXE"));
        assert!(cfg.file_eecx.should_wrap_filename("archive.tar.gz.Cmd "));
        assert!(cfg.file_eecx.should_wrap_filename("Báo cáo.v2.lnk"));
        assert!(!cfg.file_eecx.should_wrap_filename("setup.msi"));
        assert!(!cfg.file_eecx.should_wrap_filename("notes.zip"));
        assert!(!cfg.file_eecx.should_wrap_filename(".exe"));
        assert!(!cfg.file_eecx.should_wrap_filename("README"));

        // Remote merge via features.file.restricted_ext_file adds .msi while retaining defaults;
        // even if server mentions "zip", double-wrapping .zip must stay blocked.
        assert!(cfg.apply_payload(&json!({
            "error_code": 0,
            "data": {
                "setttings": {
                    "features": {
                        "file": {
                            "enable_file_eecx": true,
                            "restricted_ext_file": [".MSI", "zip"]
                        }
                    }
                }
            }
        })));
        assert!(cfg.file_eecx.should_wrap_filename("installer.msi"));
        assert!(cfg.file_eecx.should_wrap_filename("tool.bat"));
        assert!(!cfg.file_eecx.should_wrap_filename("bundle.ZIP"));

        // Disabling enable_file_eecx stops wrapping all extensions immediately.
        assert!(cfg.apply_payload(&json!({
            "error_code": 0,
            "data": {
                "settings": {
                    "features": {
                        "file": { "enable_file_eecx": 0 }
                    }
                }
            }
        })));
        assert!(!cfg.file_eecx.should_wrap_filename("setup.exe"));
        assert!(!cfg.file_eecx.should_wrap_filename("installer.msi"));
    }
}
