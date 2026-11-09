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
    ("chat", "https://tt-chat-wpa.chat.zalo.me"),
    ("group", "https://tt-group-wpa.chat.zalo.me"),
    ("sticker", "https://tt-sticker-wpa.chat.zalo.me"),
    ("alias", "https://tt-alias-wpa.chat.zalo.me"),
    ("label", "https://label-wpa.chat.zalo.me"),
    ("reaction", "https://reaction.api.zaloapp.com"),
    ("emoji_status", "https://emoji-status.chat.zalo.me"),
    ("zcloud", "https://zcld.chat.zalo.me"),
    ("file", "https://tt-files-wpa.chat.zalo.me"),
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

/// Credential-free media families confirmed in PC connect_src_rgx and upstream PR #70.
pub const MEDIA_DOWNLOAD_DOMAINS: &[&str] = &[
    "zdn.vn",
    "zadn.vn",
    "zalo.me",
    "zaloapp.com",
    "dlmd.me",
    "dlfl.vn",
];

/// Public media requests carry no authenticated session cookie.
pub const WEB_ORIGIN: &str = "https://chat.zalo.me/";

/// Zalo PC 26.11.10 playNoti public asset.
pub const NOTIFICATION_SOUND: &str = "https://stc-chat.zdn.vn/sound/noti.mp3";

/// Fork-owned release discovery; never query upstream for an ARM64 update.
pub const RELEASE_LATEST: &str = "https://api.github.com/repos/bqqhuy/ctdozlite/releases/latest";
/// Asset URLs must stay under this repository before following GitHub asset redirects.
pub const RELEASE_ASSET_PREFIX: &str = "https://github.com/bqqhuy/ctdozlite/releases/download/";

/// Credential-free GitHub release redirect allowlist.
pub const RELEASE_REDIRECT_HOSTS: &[&str] = &[
    "github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
    "api.github.com",
];

/// PC 26.11.10 getnuid adapter, command 12390.
pub const ZNOISE: &str = "https://zwid.api.zalo.me";

/// Public group invitation URLs recognized by the official join flow.
pub const GROUP_INVITE_HOST: &str = "zalo.me";

/// Allowed WebSocket hosts for real-time connection and push/pull messaging (A24).
pub const WS_HOSTS: &[&str] = &[
    "ws1-msg.chat.zalo.me",
    "ws2-msg.chat.zalo.me",
    "ws3-msg.chat.zalo.me",
    "ws4-msg.chat.zalo.me",
    "ws5-msg.chat.zalo.me",
    "ws6-msg.chat.zalo.me",
    "ws7-msg.chat.zalo.me",
    "ws8-msg.chat.zalo.me",
];

/// Validate if the given WebSocket host is an allowed Zalo endpoint.
///
/// Rejects unauthorized third-party domains, private/loopback literals, and malformed hosts.
pub fn is_allowed_ws_host(host: &str) -> bool {
    let host = host.trim().to_ascii_lowercase();
    if WS_HOSTS.iter().any(|&allowed| allowed == host) {
        return true;
    }
    // Also accept valid numbered ws shards under chat.zalo.me (e.g. ws{N}-msg.chat.zalo.me)
    if let Some(sub) = host.strip_suffix(".chat.zalo.me") {
        if sub.starts_with("ws") && sub.ends_with("-msg") {
            let num = &sub[2..sub.len() - 4];
            if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

/// Check if an IP address is private, loopback, link-local, unspecified, or broadcast (A36, A24).
pub fn is_private_or_loopback_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            if ipv4.is_loopback() || ipv4.is_private() || ipv4.is_unspecified() || ipv4.is_broadcast() {
                return true;
            }
            let octets = ipv4.octets();
            // Link-local: 169.254.0.0/16
            if octets[0] == 169 && octets[1] == 254 {
                return true;
            }
            // Carrier-grade NAT: 100.64.0.0/10
            if octets[0] == 100 && (octets[1] & 0xc0) == 64 {
                return true;
            }
            // Documentation / test ranges (RFC 5737, RFC 2544)
            if octets[0] == 192 && octets[1] == 0 && octets[2] == 2 {
                return true;
            }
            if octets[0] == 198 && ((octets[1] == 51 && octets[2] == 100) || (octets[1] & 0xfe) == 18) {
                return true;
            }
            if octets[0] == 203 && octets[1] == 0 && octets[2] == 113 {
                return true;
            }
            false
        }
        std::net::IpAddr::V6(ipv6) => {
            if ipv6.is_loopback() || ipv6.is_unspecified() {
                return true;
            }
            // IPv4-mapped IPv6 (::ffff:x.x.x.x)
            let octets = ipv6.octets();
            if octets[0..10] == [0; 10] && octets[10] == 0xff && octets[11] == 0xff {
                let v4 = std::net::Ipv4Addr::new(octets[12], octets[13], octets[14], octets[15]);
                return is_private_or_loopback_ip(std::net::IpAddr::V4(v4));
            }
            // Link-local: fe80::/10 (A35)
            if octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80 {
                return true;
            }
            // Site-local (RFC 3879 deprecated, but reserved/scoped): fec0::/10 (A35)
            if octets[0] == 0xfe && (octets[1] & 0xc0) == 0xc0 {
                return true;
            }
            // Unique local address (ULA): fc00::/7
            if (octets[0] & 0xfe) == 0xfc {
                return true;
            }
            // Multicast: ff00::/8
            if octets[0] == 0xff {
                return true;
            }
            false
        }
    }
}

/// Validate and sanitize an HTTP header value against CRLF and null byte injection (A33).
/// Removes carriage return (\r), line feed (\n), and null byte (\0) to prevent HTTP/2 and HTTP/1.1 header splitting/smuggling.
pub fn sanitize_header_value(val: &str) -> String {
    val.chars()
        .filter(|&c| c != '\r' && c != '\n' && c != '\0')
        .collect()
}

/// Check if a header value contains prohibited injection characters (\r, \n, \0) (A33).
pub fn has_header_injection(val: &str) -> bool {
    val.bytes().any(|b| b == b'\r' || b == b'\n' || b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_allowed_ws_host() {
        assert!(is_allowed_ws_host("ws1-msg.chat.zalo.me"));
        assert!(is_allowed_ws_host("ws8-msg.chat.zalo.me"));
        assert!(is_allowed_ws_host("ws99-msg.chat.zalo.me"));
        assert!(is_allowed_ws_host("WS1-MSG.CHAT.ZALO.ME"));

        // Reject untrusted hosts
        assert!(!is_allowed_ws_host("evil.com"));
        assert!(!is_allowed_ws_host("ws1-msg.chat.zalo.me.evil.com"));
        assert!(!is_allowed_ws_host("127.0.0.1"));
        assert!(!is_allowed_ws_host("localhost"));
        assert!(!is_allowed_ws_host("attacker.chat.zalo.me"));
    }

    #[test]
    fn test_is_private_or_loopback_ip() {
        use std::net::IpAddr;

        // Loopback
        assert!(is_private_or_loopback_ip("127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("127.1.2.3".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("::1".parse::<IpAddr>().unwrap()));

        // Private IPv4
        assert!(is_private_or_loopback_ip("10.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("172.16.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("172.31.255.255".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("192.168.1.1".parse::<IpAddr>().unwrap()));

        // Link-local
        assert!(is_private_or_loopback_ip("169.254.169.254".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("fe80::1".parse::<IpAddr>().unwrap()));

        // ULA IPv6
        assert!(is_private_or_loopback_ip("fc00::1".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("fd12:3456:789a::1".parse::<IpAddr>().unwrap()));

        // IPv4-mapped IPv6
        assert!(is_private_or_loopback_ip("::ffff:127.0.0.1".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("::ffff:192.168.1.1".parse::<IpAddr>().unwrap()));

        // Public IPs (should be allowed)
        assert!(!is_private_or_loopback_ip("8.8.8.8".parse::<IpAddr>().unwrap()));
        assert!(!is_private_or_loopback_ip("1.1.1.1".parse::<IpAddr>().unwrap()));
        assert!(!is_private_or_loopback_ip("49.213.76.10".parse::<IpAddr>().unwrap()));
        assert!(!is_private_or_loopback_ip("2606:4700:4700::1111".parse::<IpAddr>().unwrap()));

        // A35: IPv6 site-local (fec0::/10) and multicast (ff00::/8) guards
        assert!(is_private_or_loopback_ip("fec0::1".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("fec0::ffff".parse::<IpAddr>().unwrap()));
        assert!(is_private_or_loopback_ip("ff02::1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn test_header_injection_sanitization_a33() {
        assert_eq!(sanitize_header_value("normal_header_val"), "normal_header_val");
        assert_eq!(sanitize_header_value("header\r\nInjected: evil"), "headerInjected: evil");
        assert_eq!(sanitize_header_value("null\0byte"), "nullbyte");
        assert_eq!(sanitize_header_value("cr\r\nand\0null"), "crandnull");

        assert!(!has_header_injection("safe_value_123"));
        assert!(has_header_injection("has\rcarriage_return"));
        assert!(has_header_injection("has\nline_feed"));
        assert!(has_header_injection("has\0null_byte"));
    }
}
