//! Working endpoint table, taken from a live Zalo Web session (2026-10-21).
//!
//! Replaces guesswork with observed values. Every entry below answered `error_code 0`
//! during a real QR-authenticated session.

/// Service map as returned by `zpw_service_map_v3` on `chat.zalo.me`.
///
/// Note the domains are `*.chat.zalo.me`, **not** `*.zaloapp.com` — the PC bundle's
/// default table is stale for the web/QR session flavour.
pub const SERVICE_MAP_V3: &[(&str, &str)] = &[
    ("profile", "https://tt-profile-wpa.chat.zalo.me"),
    ("friend", "https://tt-friend-wpa.chat.zalo.me"),
    ("conversation", "https://tt-convers-wpa.chat.zalo.me"),
    ("chat", "https://tt-chat3-wpa.api.zaloapp.com"),
    ("group", "https://tt-group-wpa.zaloapp.com"),
    ("sticker", "https://tt-sticker-wpa.chat.zalo.me"),
    ("alias", "https://tt-alias-wpa.chat.zalo.me"),
    ("label", "https://label-wpa.chat.zalo.me"),
    ("reaction", "https://reaction.api.zaloapp.com"),
    ("emoji_status", "https://emoji-status.chat.zalo.me"),
    ("zcloud", "https://zcld.chat.zalo.me"),
    ("file", "https://tt-files-wpa.zaloapp.com"),
    ("voice_call", "https://voicecall-wpa.chat.zalo.me"),
];

/// `zpw_ctrs_new[0]` — conversation control plane.
pub const CONVERS_CTL: &str = "https://p5-ctl-chat.zaloapp.com";

/// `zpw_chts_new[0]` — message plane.
pub const MSG_CHAT: &str = "https://p5-msg-chat.zaloapp.com";

/// Auth host for `reqqr` / password login.
pub const AUTH: &str = "https://wpa.zaloapp.com";

/// Base host that serves `getServerInfo` for an authenticated web session.
pub const WPA_CHAT: &str = "https://wpa.chat.zalo.me";

/// Look up a service domain by key.
pub fn service(key: &str) -> Option<&'static str> {
    SERVICE_MAP_V3
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

/// Public, version-pinned Zalo Web crypto module; verified before instantiation.
pub const PHONE_SYNC_CODEC: &str =
    "https://chat.zalo.me/libs/libzproto_wasm_bg.ecc0555ba9d0f8867a4699ce943ec745.wasm";
/// Signed transfer download families, validated again after every redirect.
pub const SYNC_DOWNLOAD_DOMAINS: &[&str] = &["zalo.me", "zaloapp.com", "zdn.vn"];

/// CDN families serving direct chat photo files.
pub const PHOTO_CDN_DOMAINS: &[&str] = &["zdn.vn", "zadn.vn", "zalo.me", "zaloapp.com"];

/// Public media requests carry no authenticated session cookie.
pub const WEB_ORIGIN: &str = "https://chat.zalo.me/";

/// Zalo PC 26.11.10 playNoti public asset.
pub const NOTIFICATION_SOUND: &str = "https://stc-chat.zdn.vn/sound/noti.mp3";
