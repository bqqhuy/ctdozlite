# ADR-0001: Windows 11 ARM64 runtime and upstream integration

- **Status:** accepted
- **Date:** 2026-11-01
- **Deciders:** fork maintainer's Windows 11 ARM64 requirements

## Context

The fork targets Huawei MateBook E Go Gen 3 / Snapdragon 8cx Gen 3. Upstream v0.0.3 (`1345f5d`) adds storage, performance, update and cross-platform distribution changes that overlap this fork's DPAPI session, per-account SQLite, native media and DX12 implementations.

## Decision

Integrate upstream ancestry while retaining the Windows-specific runtime where implementations overlap. Adopt binary naming/version, local timestamp formatting, newest-row history queries, bounded initial history/cache, indexed message merging, visible-row painting, bounded bubble layout, bundled OFL Vietnamese font and lazy Windows CJK fallback. Real message identities remain `(thread_id, msg_id)`; shared names or text do not authorize history migration or deduplication. Keep DPAPI, session revocation, native tray, DX12 and Media Foundation instead of upstream's replacement storage/media paths. Do not import the multi-platform self-updater, platform release workflows, landing site or platform installation guides; the updater selects upstream artifacts and has not been validated for this fork's ARM64 releases.

## Consequences

- Only Windows ARM64 installers and portable executables are distributed.
- Upstream integration is not feature parity: settings for unsupported capabilities remain documented as incomplete.
- Phone-only 1:1 history and physical camera/call compatibility remain unverified.
- The UI close action hides to tray; explicit tray Exit terminates the application.
- SQLite schema version 2 retains the composite identity migration, rejects unknown newer versions and moves placeholder cleanup out of each hydration. Previous builds also read this composite schema; roll back the application binary without reverting/copying account databases between identities.

## Protocol evidence

CM history pagination follows `lastMsgId`, `hasMore`, `isOld`, `isFiltered`, with newest cursor `9223372036854775808`. Reference: public Zalo Web protocol reproduction in `ardennguyen/zalo-agent-cli`, commit `f867f0e992bea50d082c34cbe0637f209ee3a3c1`, `src/core/group-history.js`. Offline tests cover envelope flags, large cursors and filtered pages. This reference does not establish a supported API for old 1:1 phone messages. Group names use zero-version `getmg-v2` metadata requests; sanitized loopback fixtures verify the encrypted request and stringified response.

## Verification

Native CI must pass tests, PE ARM64/static runtime inspection, nonblank DX12 screenshots at multiple sizes/scales, hide/restore/exit against the real HWND, and installer creation. Manual verification on the target device must still cover login, old group/Cloud history, both tray mouse buttons, background delivery, outgoing calls and camera capture. No live account was used during automated checks.

## Follow-up upstream snapshot

At 2026-11-01 15:21 UTC, upstream advanced to `aad565c2d8a6528239f4b7b1990377c5e5a1348f`. Import the message/reaction/unread foundation modules and timestamp helpers with their offline tests. Retain DPAPI atomic session replacement instead of importing plaintext session writes. Exclude macOS helper packaging under the Windows-only distribution decision. Foundation-model tests do not establish live server/UI/SQLite feature parity.
