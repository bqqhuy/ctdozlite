# Queue cursor follow-up audit — 2026-11-03

Baseline: `edd27d0`, including persisted cursors (`f2fbed7`, ADR-0026).
This corrects the implementation of that decision; no new endpoint, command ID,
request field, cipher or database schema is introduced.

## Official-source comparison

Static Zalo PC 26.11.10 `compact-app-pc.5c0110a3a8749b3aacde.js`:

- `getLastActionIdsForSocket` returns both `lastId` and up to ten `preIds`, so equal
  lastId does not imply identical progress.
- `addLastActionIds`, `__markWaitId`, `saveData` and `_saveData` distinguish received
  progress from processed progress. A cursor snapshot must belong to data already
  committed, not a later in-flight page.

The public chat.zalo.me HTML was checked again and still references
`sync-v2-worker.081738ba2350940a6b28.js`, the official worker inspected in re-notes/19.
Its phone completion and conditional undelivered-scope handling are unchanged here.
Static inspection does not verify live cursor eviction/reset behavior.

## Findings and fixes

1. Refresh and periodic HTTP polling locked PollState only around receipt. Between
   persisting a batch and cloning the state, the other task could advance it. Both
   paths now share a semaphore through receipt, persistence and cursor commit.
   Each commits its own snapshot. A failed write restores the pre-cycle state so a
   later successful cycle cannot silently acknowledge the failed data.
2. Periodic polling could run before refresh loaded persisted cursors. Either entry
   path now restores progress before its first request.
3. A failed socket page was not committed, but later successful pages could advance
   past it. That queue's durable progress is now frozen at the last known good point
   for the process lifetime. Other queues continue. A process restart replays from
   the last durable position; no gap flag is saved as progress. This favors replay
   over loss when storage fails, without buffering unbounded failed batches.
4. Same-lastId pages with changed boundary IDs are persisted. Backward IDs and
   error responses do not establish new progress.
5. Cursor reads/writes run on spawn_blocking, outside the async executor. Socket
   commit passes only cursor metadata to disk work, not cloned message bodies.
   Failed disk writes remain dirty and are throttled too, with a sanitized diagnostic.
6. Valid JSON with invalid cursor values is sanitized on load. A bad entry cannot
   poison a new login; unrelated valid queues are retained.

## Verification

Regression fixtures cover equal-ID boundaries, backward pages, failed-page barriers,
error envelopes and malformed saved state, alongside the existing throttled atomic
write/read test. Full tests, strict clippy and formatting/diff checks are required.

Manual follow-up: restart with a populated database, reconnect during catch-up,
interrupt a page write, and compare received rows/cursor replay with a real account.
No executable build, packaging, commit or push is part of this audit.

Local verification completed: 492 tests passed, 0 failed, 8 ignored; strict clippy,
format check and git diff --check passed. Windows runtime behavior remains a manual
verification item; no executable/package was built.
