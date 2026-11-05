# ADR-0030: Indexed local search and explicit server state

- Status: accepted
- Date: 2026-11-04
- Extends: ADR-0012, ADR-0024 and ADR-0028.
- Evidence: `re-notes/29-v006-pc-web-contracts.md`.

## Context

Resident-message scans ran on the UI thread and Ctrl+F searched only loaded rows. The
conversation drawer showed at most nine loaded thumbnails. Server pins were confused
with local preferences, and scheduled sends were confused with server reminders.

## Decision

Schema 9 adds a rebuildable normalized search projection, external-content FTS5 trigram
index, short-transaction backfill state and transactional insert/update/delete triggers.
No history, delete replay marker, draft or phone-sync checkpoint is removed. Migration
only installs the derived schema. The first search fills missing projections in batches
of 500, releasing the write lock between batches and checking cancellation. All writers
register the projection function before using the store. Old binaries reject schema 9.

Queries execute on the worker, with debounce, a bounded result page and keyset pagination.
Account, full request and monotonic process generation guard results. At most two new
search tasks execute simultaneously; obsolete queued searches are dropped. Opening a hit
uses exact thread/message identities and bounded local context, preserving composer drafts.
The conversation library reuses these queries and existing media recovery/download/viewer
paths. Sender filters prefer source UIDs over legacy display-name columns. Deleted and
recalled content invalidates both the projection and displayed search results.

Server conversation pins are a versioned ordered snapshot, separate from local preferences.
Mutations start from a fresh snapshot, are not automatically replayed, then refetch the
server state. A failed or malformed response is never an empty snapshot. Snapshots persist
inside the account database and events carry their owner.

GROUP_BOARD routing comes from service discovery, not a guessed fallback. Server reminders
are separate from local scheduled sends. Reads/list/detail and one-shot create/update/remove
use PC-observed fields. Editing is restricted to owned, nonrepeating reminders with original
metadata. Pinned-message reads and group pin/unpin are separate from local message pins.
Mutation completion requires an explicit server acknowledgement. After a mutation, a list
refresh is required before another mutation; uncertain responses are not replayed.

UI labels use a VI/EN catalog; user messages, names and filenames are never translated.
Locale selection is applied on the UI thread. Server error text and some existing dynamic
status sentences retain their original wording pending further catalog migration.

Notification foreground policy is independent of preview visibility. Sound preparation
has a completion acknowledgement and at most three automatic attempts per enable cycle.
The Windows audio worker reuses its decoder and reports numeric playback errors locally.
Reaction requests use the PC internal kind, retain client action IDs and merge into the
existing per-reactor ledger. Recall does not erase local content before server confirmation.

## Verification and rollback

Tests cover Unicode/literal queries, hidden/deleted/recalled data, identity changes, paging,
sender identity, lazy migration/cancellation, stale account/query events, protocol params
and reaction echo reconciliation. Benchmark results are recorded in the release work note.
Windows native playback, the reported crash and new server mutations require device/account
acceptance; Linux tests do not establish those outcomes.

Rollback uses a schema-9-compatible binary or a separate pre-upgrade backup. Do not lower
user_version or delete user history. Search tables can be rebuilt by a future forward
migration. No release, tag, merge or network mutation is performed by these tests.
