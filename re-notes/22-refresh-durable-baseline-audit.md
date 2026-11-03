# Refresh delta persistence audit — 2026-11-04

Baseline reviewed independently: `33b3f6b`, atop `b3b9ef5`.
The incremental-write goal is retained. No protocol, dependency or SQLite schema changes.

## Findings

- The pre-poll list combines server previews with hydrated SQLite rows. Hashing that
  entire list falsely treats unsaved previews as durable: an unchanged preview may never
  be inserted while its queue cursor is acknowledged.
- GUI persisted the changed batch both inside poll_new_messages and in persist_received.
- A known thread with no changed message was omitted entirely, losing metadata-only edits.
- Filtering only the changed subset misses unchanged rows deleted during an in-flight
  refresh. Cache writes also preceded the final delete filter.
- Display time is reconstructed during hydration; hashing it creates false message deltas.
  A new regression test initially reproduced this unnecessary rewrite.

## Corrections and cleanup

- hydrate_refresh captures marks from actual decoded SQLite rows before merging previews.
  Partial/error baselines are discarded; uncertainty causes a safe rewrite, not omission.
- GUI receives through receive_new_messages, then persists exactly once before queue commit.
  CLI/TUI retain a persistence wrapper with disk work moved off the async executor.
- changed_since keeps thread shells and only clones changed message content. It computes
  streaming fingerprints directly instead of building another whole-map snapshot or
  allocating a JSON byte buffer for every message. Display time and transient reaction
  freshness are excluded; exhaustive destructuring requires future fields to be considered.
- GUI hydrates, hashes, filters and writes on spawn_blocking. Full-view delete filtering
  precedes persistence and follows it before the cache/UI snapshot is published.
- Delete checks use identity fields directly instead of serializing then parsing every
  message. Empty-ID identity remains msg_<timestamp>, matching persistence/deletion.
- The redundant offered/kept/dropped identity collections are removed. A failed database
  write reports that the cursor was retained rather than claiming refresh succeeded.

## Verification

New SQLite-memory regressions cover unsaved previews, renamed threads with unchanged
messages, deletion after baseline capture, zero message writes on an unchanged refresh,
and one-row recall updates. Existing cursor, receipt, delete-replay and outgoing tests
remain in the full suite. No real account/session/database was used in these fixtures.

Synthetic Linux x86_64 debug measurement: 100 threads × 20 messages, SQLite in memory,
five iterations. Full rewrite: 2,000 message writes/pass, p50 730.13 ms; delta write:
zero message writes/pass, p50 20.72 ms. This measures delta computation plus persistence,
excluding history hydration, deletion filtering, cache writes, network and rendering;
it is not a Windows startup-latency measurement.

Final validation: 496 passed, 0 failed, 8 ignored. Strict clippy, format check and
 git diff --check passed. The final diff was re-read across chat, worker and store
boundaries after the fixes. No executable build, commit or push was performed.
