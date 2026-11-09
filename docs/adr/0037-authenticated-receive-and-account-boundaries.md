# ADR-0037: Durable authenticated receives and audited client boundaries

- **Status:** accepted
- **Date:** 2026-11-09
- **Decider:** maintainer request to repair the independent audit and build a trial

## Decision

Unknown — AMBIGUOUS: ADR-0036 still governs the opt-in E2EE server experiment.
No new official prekey, ciphertext or group-E2EE contract is claimed. The changes
here close client correctness and privacy gaps without inventing those contracts.

Every low-level direct content-send API enforces the encrypted-conversation
requirement before uploading or making a request. GUI and TUI cannot bypass it
by sending quotes, stickers, contacts, forwards or attachments. There is no
plaintext retry. Explicit server encryption metadata and explicit local opt-in
remain policy inputs; a base64 body matching the synthetic envelope is not one.
Group parsing never writes a direct peer's policy or advances a 1-to-1 ratchet.

Crypto schema **8** adds `e2ee_received`, keyed by account, peer and stable message
ID. A payload fingerprint binds the receipt to the exact authenticated packet;
reusing an ID for another packet is rejected. The receipt's plaintext is protected
by the existing at-rest protection (Windows DPAPI). Receipt insertion, ratchet,
identity and consumed-prekey changes commit in one IMMEDIATE SQLite transaction.
Re-delivery with the same ID and packet returns the authenticated result without
advancing the ratchet. The raw crypto API still rejects replays under other IDs.
This receipt also recovers a message after a failed history write or a process
restart between crypto commit and history persistence.

These recovery records are user data, not disposable telemetry. History deletion,
conversation clearing/deletion and recall remove their protected recovery copies.
Cleanup failure is reported rather than hidden. A legacy successfully decrypted
history row is never replaced by a generated decryption-error placeholder, and
its identity-change warning survives that merge. SQLite connections remain on
their worker thread; the short serialized integration boundary reuses its most
recent account connection. IMMEDIATE transactions avoid WAL snapshot upgrade
failures under concurrent connections. Busy timeout is configured before WAL.

Notification snapshots carry the app lock. Locking withdraws queued previews,
avatars and activations; background publishers cannot select an account or
override its privacy lock. Hello availability is checked off the GUI thread and
the GUI stops waiting for a result after five seconds. Native-device acceptance
remains separate from fixtures.

Remembered logins create account-local protected session files and registry
entries. The account picker can retain the current login and start adding another.
Selecting a profile atomically updates the registry and revokes stale writers;
startup honors that selection only while the ordinary startup credential still
exists. Logout/expiry removal therefore cannot be bypassed by a cached selection.
An explicit logout removes that account's managed credential and registry entry.

The unsupported fork-only LAN scanner, its UI action and orphan transfer models
are removed. LAN remains unavailable until an actual contract is established.
The six friend host literals now refer to the existing endpoint constant; their
values and request payloads do not change. Bundle and device parsing fail on
malformed required fields instead of truncating IDs or inventing an empty list.

## Trial diagnostics and distribution

The bounded diagnostic writer exposes its health and a timed drain barrier.
Experimental launch refuses to enable if the initial log cannot be written;
later writer failure is visible and prevents another experimental send. Fields
remain allowlisted, including build revision, numeric codes, stages, counters and
timings. No plaintext, key, raw ciphertext, URL or arbitrary server text is logged.

GitHub workflow dispatch can build an `experimental_e2ee` artifact. It has a
separate trial artifact name, explicit build metadata and a native GUI/log smoke
case. Trial builds cannot publish a tag/release. Normal tagged builds remain
separate. This task requests a build, not a release or a version/tag change.

## Migration and rollback

Migration is forward-only and only adds the receipt table/index. Existing keys
and history remain. Schema-7 binaries reject schema 8; never lower user_version.
Back up the entire data directory with the app closed before device testing.
To stop experimental traffic, launch without its runtime flag. This does not
silently remove an existing encrypted-conversation requirement.
