# Branch audit against Zalo PC and Zalo Web — 2026-11-03

Baseline: fork `701a49c` on `fix/windows-arm64-product`. This audit preserves ADR-0024/0025;
no request fields, command IDs, crypto algorithms or database schema are changed.

## Official sources inspected

- Zalo PC 26.11.10, extracted JavaScript (not executed):
  `pc-dist/compact-app-pc.5c0110a3a8749b3aacde.js`.
  `ForwardAsync.forward`, `__callClone`, `_videoTaskWrapper`, `_forwardMessages`,
  `sendVideoMessage` and static `renewLink` confirm the copy/preparation → final forward
  separation, photo URL aliases, video/poster split, and FILE renewlink endpoint.
  A final send whose acknowledgement is missing is not evidence of failure to deliver.
- Public `https://chat.zalo.me/`, fetched 2026-11-03, declares
  `__SRC_SYNC_V2_WORKER__ = sync-v2-worker.081738ba2350940a6b28.js`.
  Worker fetched from `https://zalo-chat-static.zadn.vn/v1/`:
  3,900,426 bytes, SHA-256
  `91519f6656c81afb41f7f6989e78a6aa43beef33f75079754559b54568b33e72`.
  `checkRequestfulfilled` records each idx and sets totalBatch to idx+1 on isLast.
  The fork retains its stricter requirement that every idx in 0..last actually exists.
  `takeUndeliveredScopes` has error/type/priority/retry conditions; mere scope presence
  is not a replacement completion rule.

No authenticated account, private conversation or live media URL was used for this audit.
Static source inspection does not establish live server acceptance or iPhone behavior.

## Corrections

- Permit clone → re-upload fallback only during media preparation, never after the final
  forward request. Retain the same client identity for explicit retries.
- Construct a fresh forwarded message; do not inherit reactions, replies, pin/star state,
  unread state or history provenance from the source conversation.
- Filter delivery receipts by conversation in SQLite before LIMIT. Scoped queries use the
  existing composite primary key; unscoped IDs still require one unique outgoing row.
- Pace the actual opening of phone rounds. FuturesUnordered does not poll a future merely
  because it is inserted; sleeping between insertions previously allowed a burst of four.
- Bound MP4 box offsets without overflowing addition; never read mvhd/tkhd fields from a
  following sibling box. Malformed metadata falls back through the existing video path.
- Scope upload completion notifications by account; expire cached completions on lookup,
  reject duplicate waiters, and remove registrations when waiting futures are cancelled.
- Tie screen capture results to the account and source conversation. Stop polling on logout,
  discard results after a conversation switch, and reset the pending flag at account changes.
- Renewal flights distinguish login credentials and media references; logged-out sessions
  cannot consume cached results. Cancel in-flight/backoff polling on logout. A pending-server
  timeout is not cached as a terminal failure. Accept normalUrl-only legacy photo replies.

## Measurements and verification

Synthetic SQLite query measurement on Linux x86_64 / SQLite 3.53.1, 632,000 in-memory rows,
40 iterations, looking up a known outgoing message near the end of the table:

| Query | min ms | average ms | p50 ms | p95 ms | p99 ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Before | 33.5905 | 69.0943 | 71.9960 | 94.2023 | 99.7311 |
| After | 0.0054 | 0.0183 | 0.0059 | 0.0096 | 0.4879 |

EXPLAIN QUERY PLAN changes from SCAN messages to SEARCH using
sqlite_autoindex_messages_1 (thread_id=? AND msg_id=?). No new index or migration is needed.
These numbers measure an isolated warm in-memory query, not Windows product latency.

Regression tests cover uncertain clone delivery, source-state separation, receipt collisions,
malformed MP4 sizes/fields, lazy launch reservations, account-scoped upload notifications,
stale completions, duplicate/cancelled waiters and capture after changing conversations.
Run cargo test --locked and cargo clippy --locked --all-targets -- -D warnings.

Manual follow-up on Windows ARM64: real video/poster send and forward, capture cancellation,
account switching, expired photo renewal, and iPhone update/full-rescan/reconnect flows.
No executable/package build or publication is part of this audit.

Final local verification: 479 tests passed, 0 failed, 8 ignored; clippy with warnings denied,
format check and git diff --check passed. No Windows executable was built.

## Follow-up: warm-start and incremental synchronization

The subsequent performance report exposed three independent replay paths:

- `poll_new_messages` read 50 historical messages for every group/Cloud during refresh,
  although live/offline queues and `get-last-msgs` already supply incremental updates.
  Remove those historical calls from ordinary refresh.
- `RefreshThreads` automatically started a full server-history traversal once per worker
  lifetime, including each app restart. History backfill is now an explicit user action.
- Completed server history had no durable end marker: only intermediate continuation
  cursors were retained. Preserve completion and the server traversal's head identity in
  the existing history_sync.cursor text column as JSON. Next runs read the new prefix
  until that head is encountered. Interrupted runs preserve their continuation cursor
  and previous stopping boundary; isFiltered never marks completion. Typed legacy
  recent:/old: cursors still resume safely, without claiming earlier history was complete.
- Phone recent-only mode skipped the coverage check entirely. It now skips unchanged
  threads when completed coverage reaches the current seven-day floor and lastTs.
  Full-history mode still asks for missing older history. Missing checkpoints are shown
  in the plan, rather than inferred from nonempty message rows or zero inserted rows.

No new endpoint, inferred pagination cursor, schema migration or executable build.
Existing databases with rows but without reliable completion evidence may need one
validation/backfill pass. The optimization does not turn those rows into coverage.

Verification adds persisted server completion/update/resume/filtered/legacy fixtures,
recent-only versus full-history coverage tests, and a synthetic workload of 1,000
unchanged conversations over 40 iterations (debug Rust on Linux x86_64). The workload
asserts 1,000 planned requests before versus zero after, and prints CPU timing percentiles;
it does not predict network or iPhone wall-clock speed.

Follow-up verification: 485 tests passed, 0 failed, 8 ignored; clippy with warnings denied,
format and diff checks passed. The 1,000-conversation workload plans zero content ranges
instead of 1,000 (about 34 rounds at 30 partitions/round). Debug planner CPU p50 was
0.291 ms before and 1.204 ms after: a small CPU cost to avoid repeated phone transfers.
The metadata round remains necessary to discover which conversations changed.
