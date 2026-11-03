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
