# Typing, receipts and selected upstream PR #78 features — 2026-11-03

Static source: Zalo PC 26.11.10 `compact-app-pc` (same `app.asar` as re-notes 14/17), not
executed. Upstream PR #78 head `4a052c6` was read for scope only.

## Typing

- Receive (`_processRawMessages`): `actions[]` entries with `act_type:"typing"`; `act` is
  `typing` (1-1) or `gtyping` (group). `data` is a JSON object body without braces and is
  parsed as `"{" + data + "}"` → `{uid, gid?, ts, isPC}`. Own `uid` is ignored
  ("block send to me typing"). Indicator lifetime `userTypingTime: 8` seconds.
- Send (`sendIsTyping`): 1-1 → `getChatDomain()/api/message/typing` with
  `{toid, destType: DEST_TYPING.USER, imei}`; group → `getGroupDomain()/api/group/typing`
  with `{grid, imei}`; `functionTyping` sends at most once every 10 s.
- PR #78 instead used the conversation domain with a hardcoded fallback host and an extra
  `status: 1`; the fork follows Zalo PC.

## Delivery and read receipts

- `getDeliveredsFromData`: `delivereds[]` → `{msgId, seen, seenUids, deliveredUids}` with
  the conversation from `groupId`, `threadId` or `deliveredUids[0]`.
- `seens[]` and `groupSeens[]` travel beside `msgs`/`groupMsgs` in the same payloads
  (WebSocket and `lp_chat`).
- The fork raises the stored state of outgoing messages only (never lowers it) and shows
  "Đã nhận"/"Đã xem" on the latest outgoing message. Sending of own "seen" is unchanged.

## Verification still needed

Real-account capture of a typing action and of a `seens` entry, to confirm field names in
live traffic (the parsers accept the shapes above and ignore anything else).

## Photo quality and media forwarding

- `_shouldDowngradeOriginal` / `getSendPhotoConfig(photoResolutionMode)`: Zalo PC resizes
  photos client-side in the default HD mode and keeps the file only in ORIGINAL mode; the
  `photo_original/upload` and `photo_original/send` requests are the same. PR #78 instead
  toggled `isHD`/`hd`/`isOriginal` request fields; no evidence was found that the server reads
  them, so the fork changes only the uploaded file (HD copy: long side 2560 px, JPEG q90,
  EXIF orientation applied; GIF untouched). Default stays "original", as before.
- Forwarding media: Zalo PC clones media for the target with `renewlink`, and falls back to
  `_reuploadMedia`. The fork uses the re-upload path only (download the original into a
  private folder, send through the existing attachment flow, delete the copy). Videos are
  re-sent through the same attachment flow as files.

## Video messages and media forwarding like Zalo PC (supersedes the re-upload note above)

Static source: same `compact-app-pc` bundle.

- Video upload (`uploadVideoMessage` → `uploadVideo` → generic uploader): chunks go to
  `getFileDomain()/api/{message|group}/video/file/upload?…&params=…&type=2|11` with
  `{totalChunk, fileName, clientId, totalSize, imei, chunkId, toid|grid}` and form field
  `chunkContent` (`buildSendChunksInfo`). A video chunk is done when `data.finished` and
  `data.fileId`; the CDN URL arrives later (`fileAsync.fileUrl`, the `video_done` control).
  The poster goes to `video/thumb/upload` the same way and is done on `photoId`; its URL is
  `fileAsync.oriUrl || hdUrl`.
- Sending (`sendVideoMessage` → `_forwardMessages(…, MSGTYPE_FORWARD_VIDEO)` → `forwardFile`
  → `forwardMessage`): `getFileDomain()/api/message/forward` `{toId, imei}` or
  `/api/group/forward` `{grid, visibility}`, plus `msgType`, `clientId` and
  `msgInfo = JSON {videoUrl, thumbUrl, duration (default 1), width, height, fileSize,
  properties, title}`. `MSGTYPE_FORWARD_FILE/PHOTO/VOICE/GIF/VIDEO = 1/2/3/4/5`.
- Forwarding (`_forwardMessages`): photo `msgInfo {title, oriUrl, thumbUrl, rawThumbUrl,
  hdUrl, previewThumb, width, height, properties, fileSize, hdSize, thumbSize}` (requires
  `oriUrl` and `thumbUrl`); file `{url, fileName, extension, checksum, size, fType,
  fdata?, thumbUrl?}` (refused unless `checksum` is 32 hex digits).
- `forwardFile` with `async_forward` → `ForwardAsync.forward(target, msgType, msgInfo,
  newClientId)`: `renewLink(target, msgType, msgInfo, newClientId)` (msgInfo gains
  `isE2EE`, `isOriginal`), then `_transContent` applies the copied URLs (photo `normal/hd/
  thumb` → `oriUrl/hdUrl/thumbUrl`, `normalUrl` cleared; file `fileUrl` → `url`; video:
  one copy with `renewType: 1` for `videoUrl`, one with `renewType: 0` and `isPhotoOriginal`
  for `thumbUrl`) and calls `forwardMessage`. `_reuploadMedia` is the fallback.
- The `isClone` flag (`_forwardMessages`) depends on remote config (`media_cloud`) and was
  not reproduced. Live captures are still needed for the reply of `video/file/upload` and
  for one forward of each media kind.
