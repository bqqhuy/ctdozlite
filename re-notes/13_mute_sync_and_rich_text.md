# Account-wide mute sync and incoming rich text

Selective upstream port, 2026-11-02:

- `ctdoteam1110/ctdozlite@9328cc9` (#67): mute snapshot, mute mutation and realtime mute controls.
- `ctdoteam1110/ctdozlite@ed550f6`: incoming `rtf` text styles.

## Evidence and limits

Upstream derived the wire contract from the official Zalo PC bundle (`getMuteConversations` cmd `11280` in `default-login-main-startup-shared-worker.js`, `setMuteConversation` cmd `11288` in `compact-app-pc.js`, `_processMuteConversations` in `app.asar`). This fork did **not** capture fresh authenticated traffic; the fixtures in `src/mute_sync.rs` and `src/rich_text.rs` are synthetic and follow the upstream shapes. Real server interoperability remains a manual check (see ADR-0013).

All requests use the existing path: `encoded_params` (ParamCipher v2) → `common_params` signing → shared session client, host `domains.profile` from the service map.

## Mute snapshot

`GET {profile}/api/social/profile/getmute?{common}&params=enc({"imei"})`

Decrypted `data` (possibly string-encoded):

```json
{"chatEntries":[{"id":"<uid>","duration":-1,"startTime":1700000000,"muteMode":1}],
 "groupChatEntries":[{"id":"<gid>","duration":3600,"startTime":1700000000,"muteMode":1}]}
```

- `muteMode == 1` and `duration != 0` means muted; `duration == -1` is "until turned back on"; positive duration expires at `startTime + duration` (seconds; millisecond values are normalized).
- Both lists must be present. A response missing either list is rejected and local state is kept.

## Mute mutation

`POST {profile}/api/social/profile/setmute?{common}` body `params=enc({...})`:

```json
{"toid":"<id>","duration":-1,"action":1,"startTime":<unix>,"muteType":1,"imei":"..."}
```

`action` 1 mutes, 2 unmutes; `muteType` 1 peer, 2 group. The fork only sends `duration: -1` (the menu has no timed mute). The response must pass `confirmed_mutation`; otherwise the local flag is not changed.

## Realtime controls

WebSocket `601`/`602`/`603` and `lp_control` payloads carry `controls[].content` with `act_type: "mute"`, `act: "set" | "unset"` and `data` (object or JSON string) containing `toId`, `muteType`, `duration`, `startTime`, `muteMode`. Expired timed mutes are treated as unmuted. Non-numeric IDs are ignored.

## Incoming rich text

Official clients send styled text as `content = {"action":"rtf","title":"<text>","params":"{\"styles\":[{\"start\":N,\"len\":M,\"st\":\"b,i,u,s,f_18\"}],\"ver\":0}"}`. Offsets are UTF-16 code units. The fork keeps `title` as the exact message text and stores validated spans in `ChatMessage::text_styles`; out-of-range, zero-length and unknown tags are dropped; at most 256 spans. `f_*` (font size) renders as emphasis. Upstream instead rewrote the text with Markdown markers, which changed copied/quoted text; that approach was not taken. Outgoing Markdown-to-RTF conversion was not ported.
