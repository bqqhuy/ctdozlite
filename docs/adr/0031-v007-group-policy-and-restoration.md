# ADR-0031: Group policy, message lifecycle and explicit restoration

- **Status:** accepted
- **Date:** 2026-11-05
- **Deciders:** repository maintainer, implementation assistant

## Context

v0.007 adds group management and two explicit restoration paths. These must not
weaken delete replay markers, account isolation, message identity or Sync2 coverage.
Protocol evidence is recorded in `re-notes/31-v007-contracts.md`.

## Decision

Schema 10 installs derived expiry indexes, conversation policy snapshots, durable
reaction inbox/preview state, restoration asset references and metadata/history
freshness state. Migration preserves messages and checkpoints. Expiry backfill and
purge use bounded batches; expired rows produce content-free delete replay markers.
The timestamp is the retained server timestamp plus a millisecond TTL, never the
local receipt time. History applies expiry before its result limit.

Group sends fail closed when membership/permission cannot be verified. Shared
account/runtime-scoped snapshots coalesce concurrent checks for a short freshness
window. Opening a group requests its send policy separately from message history.
Mutations require explicit acknowledgements and do not replay uncertain writes.
Notes, polls and reminders retain distinct observed service contracts.

Incoming reactions persist before a queue cursor is acknowledged. Missing targets
remain pending; the existing per-reactor ledger gives idempotent replay. Preview
activity does not manufacture a new message, unread count or message timestamp.
All changed targets in one drain are merged into the UI update.

Desktop V2 is a user-selected, account-bound experiment with a dry run followed by
explicit import. AES-CBC/tar structural checks are not authentication. A synthetic
OpenSSL fixture tests the format independently of the Rust encoder. Media goes to
a durable account-local restoration vault. The importer uses the fork's identities
and delete replay markers rather than copying upstream's SQL importer.

Import commits at most 250 records per transaction. Cancellation rolls back the
current batch; earlier committed batches remain and an idempotent rerun continues.
This deliberately replaces the earlier whole-import rollback proposal, which would
hold the SQLite writer lock throughout a large restore and stall live messages.

V1 fallback is explicit and experimental. RSA private material stays in task memory;
control events must match the current public key and computer identity. Download,
decompression and import are bounded and cancellable. ID mapping uses znoise rather
than a hardcoded self UID. This path never advances Sync2 coverage.

## Consequences

More derived local state must be maintained. Old binaries reject schema 10. Offline
policy refresh retains the last confirmed account snapshot. Protocol fixtures do not
replace Windows ARM64/iPhone testing. V1 BinNet payloads are preserved but not fully
decoded; JPEG XL restoration uses a bounded pure Rust decoder and a Chromium test fixture.
XZ uses a streaming decoder with a 128 MiB decoder/dictionary budget; decoded output
is separately capped. Rich group control updates, ordinary media-cache expiry and
Windows device behavior still require review before release acceptance.

## Dependency review

`rsa` is pinned to 0.10.0-rc.20 for the explicit V1 RSA-2048 flow; its prerelease
status remains a release-review item. `rand` supplies the system-seeded CSPRNG.
`jxl-oxide` handles local Desktop JXL images under an allocation tracker.
`lzma-rust2` supplies streaming XZ with a pre-allocation dictionary budget; the
previous lzma-rs candidate buffered whole blocks before enforcing the output cap.
Both media decoders avoid new native C toolchains on Windows ARM64. `tar` only
reads validated regular files; `tempfile` owns private staging lifetime.
All resolved versions are in Cargo.lock. `cargo audit` is not installed in this
workspace; the repository's required audit must run in release preparation/CI.
