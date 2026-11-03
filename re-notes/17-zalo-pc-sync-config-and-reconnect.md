# Zalo PC sync configuration, wake-up and reconnect — 2026-11-03

Static source: Zalo PC 26.11.10.3400 installer supplied by the maintainer (7 volumes), not executed.
Installer SHA-256 `7166ea28925b3121369ffb25e87b4749a648f47a861272009dcb14094b5a5511`, `app.asar`
SHA-256 `70f1b53c4db61321bf1b11c722a9049823711328b87e17b0d19338f83a5f6ed3` (same as re-notes 14).
Extracted files stay outside the repository.

## Manual-sync defaults (`pc-dist/lazy/main-startup.*.js`, `getSyncRemoteConfigs`)

These are client defaults; the server's remote config can override them at runtime and its
live values were not observed.

| Key | Default |
|---|---|
| `manual_sync.sync_conv_list.max_time_range_d` / `max_number_of_conv` | -1 / -1 (unlimited) |
| `manual_sync.sync_last_chat` | 10 conversations × 1 message |
| `manual_sync.sync_recent_chat` | 10 conversations × 100 messages |
| `manual_sync.sync_all_remaining_msg` | range -1, conversations -1, messages -1 |
| `awaiting_mobile_timeout_ms` | 600000 |
| `wakeup_timeout` | 180000 |
| `batch_timeout` | 10000 |
| `sync_paused_timeout_ms` | 600000 |
| `sync_keep_batch_wait_across_idle` | true |
| `max_number_of_concurrent_sync_items` | 4 |
| `max_percent_batch_fail` | 0.5 |
| `enable_auto_request_sync_on_log_in_new_device` | false |

`createThresholdRange(days)` turns a positive day count into `fromTs = midnight(now - days)`;
`-1` means `fromTs = 0`. So Zalo PC applies no history window by default and splits work into
conversation list → last chat → recent chat → remaining messages.

## Wake-up (`sync-v2-sub-worker`, Zalo Web worker)

`session.wakeup()` sends `requestWakeUp({syncId, toDevice})` (WS command 592). On code 0 the
batch wait restarts with `wakeupTimeout`. The fork sends
`{"data":{"syncId":...,"toDevice":0},"reqId":"req_wake_<syncId>"}` and matches the reply by
`reqId`. A real-device acknowledgement still needs to be captured.

## Field logs (maintainer, 03/11)

- After control-queue fixes: 19 message rounds completed with `isLast` and every index,
  about 632 000 messages in about 11 minutes; the largest round had 322 batches.
- One run stalled with indices 121–141 missing; the phone reported Idle then Active together
  after 150 s of silence, consistent with iOS suspending Zalo.
- The last run lost its WebSocket right after round 20 was confirmed. Disposal failed
  (`socket_write_ack: false`) and no cleanup ran on the reconnected socket, leaving the
  iPhone on "Đang đồng bộ". The close code was not logged at the time.

## Async file upload (`compact-app-pc`)

Controls grouped by `act_type` dispatch `file_done` and `video_done` to
`_processFileAsyncDoneEvent` / `_processVideoAsyncDoneEvent`, which accept `{fileId, data:{url}}`.
`sendMsgFileAsync` then posts `/api/message/asyncfile/msg` (11777) or `/api/group/asyncfile/msg`
(13001) with that `fileUrl`.

## Upstream queue evidence

Upstream PR #69 (`58ea93f`, real account 02/11): `lp_chat` repeated `more = 1` for queue `16`
(511_1) 1799 times in 3 minutes while only queues 1/2/3 were sent; adding 15/16 was accepted.
