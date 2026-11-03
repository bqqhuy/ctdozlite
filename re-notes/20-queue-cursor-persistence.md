# Queue cursors across starts (Zalo PC 26.11.10) — 2026-11-03

Static source: `compact-app-pc` (same bundle as re-notes 14/17/18), not executed.

## Long polling (`lp_chat`)

- The multi-queue id manager starts at `lastActionId = "1"`, `lastActionIdOther = "1"`, then
  `init(uid)` reads `KEY_ACTION`, `KEY_ACTION_OTHER` and `KEY_MSG` (per-queue `lastId`) with
  `getItemForCurrentUser` / `getObjectForCurrentUser`.
- `addLastActionIds` advances with `MessageIdComparator.max` and stores a pending snapshot
  (`saveActionIdData.set(localId, …)`, `__markWaitId`); `saveData` writes it after the page
  was processed. A reset removes the three keys.

## Socket offline queues

- `doGetOffline(cmd, subCmd, first)` sends `{first, reqId, lastId, preIds}`; unless the queue
  is in `socket.none_ids_queue`, `lastId`/`preIds` come from
  `getLastActionIdsForSocket("<cmd>_<subCmd>")`, which returns `{lastId: "1", preIds: []}`
  only for a queue it has never seen; `preIds` are at most 10 ids.

## Fork before this change

`PollState` was never stored and every socket pull sent `lastId: 1`, so each start (and
each reconnect) replayed every queue from its beginning. Field logs showed such catch-ups
ending with 0 new rows.

## Fork now

Per-account `<history>.queue-cursors.json` holds the `lp_chat` state and `(lastId, preIds)`
of socket message queues 510/511/513–518 (control queues unchanged). A cursor is committed
only after the page's messages were written to SQLite; writes are throttled to once a
second and forced at the end of a burst and at logout. Missing or damaged file: start from
`1` as before. Server eviction of a too-old cursor still needs a live capture.
