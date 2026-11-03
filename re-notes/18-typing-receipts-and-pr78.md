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
