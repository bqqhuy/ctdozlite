# Group SFU integration evidence (2026-11-02)

Source: ctdoteam1110/ctdozlite `23f06b7`, reviewed at immutable snapshot `20e742c098ec546515701dcf1efe6b92fe9ea8ee`.

- `src/call.rs`: `build_zrtp_group_init_packet`, alternate init v6, group ping/end, SFU layer/subscription packets; `collect_relay_servers` and invite-builder nested `callSetting`/probe-selected relay handling; group request and member state control.
- `src/media.rs`: RFC 5285 group audio/video header extensions and SFU room/layer/subscription handling. Existing source comments reference official `ZaloCall.exe` offsets `0x0092E8FB`, `0x00893ED5`, `0x00924325` and RTP extension map `0x00A77943..0x00A77ED1`. These are inherited reverse-engineering evidence, not a fresh capture by this fork.
- `upgrade_session_for_group_call`: upstream signed `getLoginInfo` request uses protocol type 24/version 665. Fork requires an explicit successful envelope before retrying group allocation.

Fork adjustments: retain Media Foundation; explicit group-mode flag rather than port heuristics; only advertise the actually encoded 320x240 base layer; abs-send-time derives from NTP; per-SSRC Opus decode/mix and two independent remote video decoders. Group allocation failure does not initiate separate calls to all members. Default diagnostics use the existing redaction path.

Offline verification uses synthetic IDs, RFC 5737 addresses and mock session tokens. Tests cover nested metadata, choosing a nonfirst probed relay and matching RTCP/token metadata, exact init/ping/end/subscription fields, actual layer count, RTP extension/payload boundaries and mixing two real Opus streams without doubling playout duration.

No account login, call, camera capture or new traffic capture was performed. On MateBook E Go Gen 3, verify outbound/inbound group audio/video, local camera off with remote video still visible, member join/leave, group allocation rejection, cancellation, NAT reconnect and explicit tray Exit during calls. Observe server error_code and sanitized diagnostics; do not attach raw session material.
