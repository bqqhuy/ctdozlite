# 74 — Voice, calls and zCloud reference evidence

Date: 2026-11-24, Asia/Saigon. Scope: reference research, no application code changes.

## Source and evidence boundaries

Installed PC: 26.11.12, read-only `resources/app.asar` extraction in memory.
Character offsets below are Python Unicode string offsets, not byte offsets.

| Asset | SHA-256 |
|---|---|
| `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js` | `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08` |
| `pc-dist/lazy/main-startup.41abf8e6af2a55551e3b.js` | `c29c9f0a7f0718c5b044764666b6fc5dec8b8eb65e178819b3cee3b1f93e3426` |

E2 means static implementation, not successful server acceptance. `UI-LIVE` below
records observed official-client behavior; it is E1 for that UI scenario only,
not an E1 wire capture. No HTTP/WebSocket payload capture was collected in this
session. No credentials, raw user messages, voice bytes or camera images are saved here.

## PC voice sending implementation — E2

Main asset, CP 15,098,609, `static sendVoice(e,t,n,a)`:

- DM resolves `getFileDomain()` and `/api/message/voicemp3`, command 11148.
- Group strips `GROUPID_PREFIX`, resolves the same file domain and
  `/api/group/sendmp3`, command 11169.
- DM serialized params contain `toid` and `clientId`; group contains `grid` and
  `clientId`. Exact case matters.
- Query adds common params, percent-encoded `encodeAES(serialized_params)`, and
  `type=2` for DM or `type=11` for group.
- Body is `FormData` with field `fileContent` holding `t`.
- Wrapper invokes `_post` with command ID, credentials, timeout 60,000 ms and an
  explicit form-urlencoded header setting. The actual multipart boundary/content
  type on the wire depends on the wrapper/adapter and is NOT established by this
  call alone. Inspect that wrapper or capture before reproducing headers in Rust.

This proves more than route existence, but does not establish the file codec,
filename/MIME, duration unit, accepted response schema, E2EE support, current
entitlement or an active recording UI. A bounded search found `sendVoice`
definitions in shared bundles but no `.sendVoice(` caller in the inspected main
and main-startup assets. Do not assume it is a currently exposed recorder.

## zCloud implementation — E2

Main asset, CP 18,846,200 onwards, class `L`:

- `getCloudViewerKey()` uses AUTH domain, GET, common query params, no per-file
  `file_id` body; result supplies `viewer_key` and `enk`, cached by the PC client.
- `encodeParams(e)` encrypts JSON using cloud-specific `enk`.
- `verifyCloudMedia(t,n,a,i)` builds
  `{lastNoiseId:t, loadType:n, listNoiseIds:a, tracking_source:i}`. Command selection
  depends on the plan. URL uses media cloud domain, common query and cloud-encrypted
  params. Numeric enum values/defaults must be resolved from their definitions;
  do not invent them from the parameter names.
- `getDownloadUrlByNoiseIds(t)` requires a nonempty list and builds
  `{list_noise_ids:t}` with cloud-encrypted params, not `{file_ids,imei}`.
- `getUsageMyCloud()` uses `/cloudmedia/info/pc/v1/usage-mycloud` and the cloud
  request executor `N`; `getUsageZCloudV2()` is separate.
- Route table near CP 1,620,320 registers verify, viewer-key and download routes.
  It is the implementation above, not this route table, that supports these shapes.

Still inspect executor `N` and its helpers for viewer credential placement, HTTP
method, response decryption/envelope, refresh/retry and error semantics. Do not
infer absence/expiration from a failed verification HTTP request.

## Reference UI observations — UI-LIVE

### Web

The already authenticated `chat.zalo.me` account was opened through the in-app
browser. My Documents and the authorized DM were inspected. In this session:

- Composer toolbar and More showed no recording entry. DM header showed no
  voice/video call buttons. This is an account/build observation, not a universal
  claim that Web never supports them.
- Rail `Zalo Cloud` opened an informational popover with `Tìm hiểu zCloud ngay`.
  No subscription, purchase or cloud reset was attempted.
- My Documents is a conversation for cross-device storage, not evidence that the
  paid zCloud product and self-conversation are interchangeable.

### PC and device preflight

User signed in and selected the correct Henry test conversation after multiple
same-name search results were found. Calls were made only to that selected contact.

- Header has separate phone and video buttons, followed by search and info.
- First voice call opened `Tình trạng thiết bị`: microphone status, selector,
  live level dots; speaker/headphone status and selector; speaker test control;
  volume slider; bottom `Hủy` and primary `Bắt đầu gọi`.
- During the call, settings reopened the device panel with `Hủy` and without the
  preflight Start button. This matters for avoiding accidental redial.

### Voice call

- Dedicated native `Zalo Call` window, portrait layout in this observation.
- Large centered circular avatar over blurred avatar background.
- Green elapsed timer at top left once the timed phase was visible.
- Bottom dark dock: camera with chevron, prominent red hangup, mic with chevron,
  gear at right. Some controls were disabled during initial ringing.
- Initial AX status `Đang đổ chuông ...`; subsequent AX exposed elapsed time.
- Hangup closed the call window. Timeline showed outgoing voice call card,
  duration 42 seconds and `Gọi lại`.

### Video call

- Started from the distinct header video button. Initial AX said `Đang kết nối ...`.
- Landscape call window, received live changing video, local preview at top right,
  participant labels, elapsed time at top left, dark bottom controls.
- Main image preserved portrait aspect with side space; do not stretch every video
  to fill the landscape surface.
- Bottom controls included screen-share icon, camera/chevron, red hangup,
  mic/chevron, grid/layout icon and gear. Presence is not acceptance of every action.
- Camera chevron opened `Chọn camera`: front/back device names, checked selection
  and a live preview in the same popup. No device selection was changed.
- Hangup closed the video window; timeline showed outgoing video card, 18 seconds
  and `Gọi lại`. Window inventory then contained no active call window.

These are official-PC UI observations. They do not validate fork audio quality,
fork codecs, fork signaling, group calls, screen share or audio-to-video upgrade.

### Voice bubble and AI

User supplied a short test voice message after the calls:

- Incoming rounded white bubble with separate blue play control, amplitude strip,
  displayed duration 00:04 and small cloud indicator, AI control on the right.
- Play advanced the position/blue portion of the strip inline; completion changed
  the control to replay. No external player window opened.
- AI click displayed recognized test text below the strip inside the same bubble;
  a chevron replaced the collapsed AI control. No transcript content is retained here.
- Hover exposed reply/share/more controls outside the bubble. Menu contents were
  not successfully inspected, so their availability must not be claimed.

### zCloud in the timeline

My Documents and the test DM displayed `Đã có trên zCloud` under some file names,
beside size/status, with folder/download controls. No file ID input or credential
viewer was involved. No cloud file download/decryption was tested.

## Relation to earlier evidence

- [47](47-pc-native-voice-playback-ai.md): AI request `{url}`, `ai_features`,
  command 13394 and asynchronous `req_id`/`err` result correlation.
- [08](08_video_call.md): chronological captures. Read the fifth-capture corrections
  before using earlier hypotheses: H.264 key/delta PT 98/97, proper relay RTCP
  types, both-socket RTP receive and TWCC extension ID 5. Current source uses native
  Media Foundation; older FFmpeg/synthetic-only notes are not current status.
- [09](09_group_sfu_arm64.md) and ADR-0003: bounded group media implementation,
  no claim of live group interoperability.

Parity with Zalo PC/Web (source: re-notes/74) applies only to the specific UI
observations and static shapes above. Outgoing codec/response, upgrade handshake
and full cloud executor contract remain Unknown — AMBIGUOUS.
