# Full Code Audit — 2026-11-19

## Scope and method

Independent static review of the project at `main` / `origin/main` `50d6abe`,
plus review of the current uncommitted audit diff. No source changes were made by
this auditor. Repository guidance, security/concurrency/database/API standards,
the Zalo reference authority, Spec Kit index, master action plan and prior audit
reports were read. The code inventory is 159 Rust files under `src/`, 13 Rust
integration-test files, 12 feature spec directories and 47 `re-notes` documents.

This is a first-pass audit, not proof that each Rust statement or each ASAR/native
binary was exhaustively reversed. Findings below require cross-review by the PC
parity auditor. E1 device acceptance and undocumented server behavior remain
explicitly unknown.

## Confirmed finding (cross-reviewed)

### P1 security invariant — WebSocket configuration permits sending the session cookie over plaintext

`Session::apply_server_info` accepts endpoint strings beginning with either
`wss://` or `ws://` (`src/session.rs:1137-1145`). It retains the raw URL in
`domains.ws`. Restoring a saved session also deserializes that vector directly;
the REST-only `prefer_verified_rest` sanitizer does not validate WebSocket
scheme. `socket::run_once` builds a request from the endpoint and checks the host
and DNS, but not the URL scheme (`src/socket.rs:746-754`). It then inserts both
`Cookie: zpw_sek=...` and a `zpw_sek` header (`src/socket.rs:760-766`). Therefore,
if an insecure endpoint enters the list, the authenticated handshake can expose
the session cookie in cleartext. The host allowlist limits the destination to
official WS shard names, but does not provide transport confidentiality against
DNS/on-path redirection when the scheme is `ws`.

**Precondition / severity:** normal login configuration is fetched over HTTPS,
so this review does not establish a remote downgrade injection. The path is
reachable if server configuration supplies `ws://`, or a saved configuration is
poisoned. The two auditors agree on P1 for the credential transport invariant,
with this conditional trigger and no demonstrated config poisoning or live
exposure. The code violates the `wss://` contract recorded in
`re-notes/02_chat_data.md:161` and should fail closed. TLS-delivered config and
DPAPI storage protect the origin but do not enforce transport safety at use.

**Suggested correction:** require `wss` both when accepting login configuration
and immediately before the socket handshake (including persisted values and the
`ZCALL_WS` override). Parse and validate the complete URI, not only its host,
before attaching credentials. Add regressions proving `ws://` config and override
cannot reach the credential-bearing handshake while known `wss://` shards remain
accepted. The parity auditor independently confirmed the data path and accepted
this finding with the conditional-trigger qualification. Repair agent added a
final `secure_ws_request` scheme guard before DNS/auth and an ingest filter;
both auditors accepted the minimal diff and offline regressions. Parent records
execution results.

### P1 — Raw SQLite main-file rollback can lose pre-import history

`desktop_backup.rs:387-393` created an `AtomicRestoreSession` before opening the
WAL store. The helper copied only the main DB (`phone_sync/import.rs:327-355`),
without its WAL. The import commits batches every 250 records, but on error it
copied that snapshot over the main database while the store/current transaction
were still open, then dropped the transaction (`desktop_backup.rs:515-519`).
This contradicted ADR-0031 and the function's documented partial-batch retention.

An isolated Python SQLite reproduction used only a temporary DB: checkpoint the
schema; commit a pre-import row in WAL; copy the main DB; commit an import row;
checkpoint/truncate WAL; begin a new row; raw-copy the snapshot; rollback and
reopen. Both committed rows disappeared while `integrity_check` still returned
`ok`. Without the intermediate checkpoint they survived. Loss therefore depends
on checkpoint timing rather than the intended transaction policy. No account DB
was accessed. The parity auditor accepted the source trace and conditional P1.

Repair agent removed the snapshot path/helper and uses SQLite transaction
rollback before deleting current-batch media. Auditor review accepted this diff
and a real `desktop_backup::execute` encrypted-fixture regression covering WAL
checkpoint, second-batch failure, existing rows, committed/current batch assets,
pre-cancel and idempotent rerun. Parent test execution remains authoritative.

### P2 — Playback cache uses two atomics for one combined capacity invariant

`video_player.rs:reserve_capacity` conditionally increments pending bytes while
reading admitted bytes; `admit_unreserved` does the reverse. Concurrent 400 MiB
operations can each observe the other counter as zero, succeed on different
atomics, and exceed the 512 MiB combined cap. `admit` also subtracts pending bytes
before adding admitted bytes, exposing an undercount interval. Acquire/release
ordering does not make two locations one transaction. Both auditors accepted this
logical interleaving. A repair with one budget mutex and isolated concurrency
regressions is requested; this is local resource safety, not a PC wire contract.

### P2 — Legacy Hello-only app-lock state starts a restored session unlocked

`AppLockGuard::new` initializes `biometric_available=false` and starts an async
probe (`gui/auth_guard.rs:36-76`). The GUI constructor locks only when a PIN is
present or the probe already reports Hello available (`gui.rs:1633-1637`), which
cannot occur at that point. `update` polls the probe but only applies the
five-minute idle lock policy (`gui.rs:6322-6333`), so availability success does
not restore the startup privacy lock. Preconditions are a logged-in restored
session and legacy/configured `enable_app_lock=true`, `use_windows_hello=true`,
no PIN. Current settings UI requires PIN setup when enabling lock, so this is not
a universal new-user bypass. Both auditors accept the conditional legacy-state
finding. A safe repair must hold privacy while probing and offer recovery/logout
if Hello is unavailable; disabled users must not be forced into Hello/lock.

### P3 — Duplicate startup deduplication

The GUI constructor called `deduplicate_threads(&mut threads, None)` twice in
succession with no intervening mutation. The second pass is redundant startup
work; parent removed it after source review. No protocol/state behavior changes.

## Confirmed implementation gaps (priority depends on accepted MAP scope)

- `src/gui/modals.rs:4368-4369` has an explicitly commented-out album position
  counter and a TODO to wire the `MediaNeighbor` response. Previous/next controls
  exist nearby, so navigation is present but current/total album position is not.
  This is an unfinished UI detail, not dead code.
- `src/media_download.rs:852` marks `range_request` as not wired to the video
  player. The endpoint helper exists, but its streaming path is not connected to
  playback; large/seekable video behavior remains incomplete. Confirm against
  the relevant MAP/spec and PC bundle before treating this as a parity defect.

## Coverage ledger

The complete file inventory and actual read ranges are recorded in
[`full-code-file-ledger.csv`](full-code-file-ledger.csv). An inventory-only entry
is deliberately not counted as semantic review or full acceptance.

| Area | Reviewed sources / evidence | Result and limit |
| --- | --- | --- |
| Session, auth and request path | `session.rs`, `login.rs`, `qr.rs`, `endpoints.rs`, `service_resolver.rs`; API/security standards | Found WS scheme gap above; did not capture live login/config. |
| WebSocket and queue | `socket.rs`, `socket/queue_receiver.rs`, `queue_catchup.rs`, `queue_cursors.rs`; E2 `re-notes/02_chat_data.md` | Bounds, host allowlist and queue ownership inspected; no end-to-end live push/reconnect acceptance. |
| Message/send/media | `chat.rs`, `outgoing.rs`, `upload.rs`, `media.rs`, `media_download.rs`, `media_renew.rs`, `media_recovery.rs`; PC notes 15/22/49 | Existing audit diff changes low-level link E2EE guard; its tests are separately reviewed by parent. Streaming player TODO remains. No live upload/renew contract confirmation. |
| Local storage and account boundary | `storage.rs`, `store.rs`, `profiles.rs`, `account_controls.rs`, `attachment_cache.rs`; DB/security standards | Prior report provides focused cache/account samples; migration/crash consistency not exhaustively traced. |
| Phone sync and import | `phone_sync/{assets,codec,import,legacy,pipeline,plan,policy,transfer,wire}.rs`; E2 source references | Inspected import identity/quarantine and bounded download/cleanup paths; did not replay a real device transfer or audit every rollback interleaving. |
| GUI and worker wiring | `gui.rs`, `gui/worker.rs`, `gui/workspace.rs`, `gui/modals.rs`, tools panel; concurrency/API standards | Current diff adds runtime/thread provenance for history/todo/poll. Album counter TODO found. Full per-control UI-to-worker reachability is not certified. |
| VoIP/native Windows | `call.rs`, `call_lifecycle.rs`, `windows_video.rs`, `video_player_windows.rs`, `desktop_notice_windows.rs`; saved PC bundle inventory | Static coverage only. No ARM64 hardware test, camera/audio lifecycle test or native ABI review. |
| E2EE and cryptography | `crypto/*`, `e2ee_integration.rs`, `e2ee_trial.rs`; security standard and handover | Experimental contract remains gated per docs; no live interoperability or exhaustive crash/ratchet atomicity verification. |
| Backup, updater and diagnostics | `desktop_backup*.rs`, `updater.rs`, `logger.rs`, `transfer_diagnostics.rs` | Prior report sampled selected guards; no full filesystem race or packaging audit here. |
| Dead code / performance | static TODO/FIXME/unsafe/request/task inventory | No project-wide deletion recommendation is supported. `cargo check`/Clippy cannot prove dynamic consumers absent; performance tests are synthetic and several are ignored/manual. |

## Current audit diff

The working diff contains changes in `chat.rs`, `gui.rs`, `gui/tools_panel.rs`,
`gui/worker.rs`, `message_view.rs` and `tests/audit_regressions.rs`. This auditor
read the diff but did not independently rerun tests; parent recorded full tests,
check, Clippy and diff-check as passing for this snapshot. No objection to the
diff has been established in this first pass; the WS finding is outside that diff.

## Cross-review status

The Zalo PC parity auditor independently confirmed the WS downgrade finding and
accepted P1 invariant wording with conditional trigger/no live exploit proof.
Both reviewers also accepted the SQLite data-loss trace, cache-budget race and
legacy Hello-only startup-state findings with the qualifications above.
Both reviewers agree album counter and range wiring are documented scope gaps,
not independently proven PC mismatches or justified dead-code removals. The PC
`videoTaskWrapper` reference concerns clone/renew logic, not proof of player range
behavior. No remaining disagreement on these three items. This is finding-level
consensus, not project-wide acceptance or a claim that no further bugs exist.

## Additional parity gap: logged-in device management

The installed PC bundle scan for versions 26.10.20 and 26.11.12 found no literal
`/api/login/getdevicelist` or `/api/login/kickdevice` route. The matching
`getDeviceList` symbol refers to native ZCall audio/video devices, not logged-in
account sessions. The `actlistv2` call belongs to the pre-login device selection
flow and does not establish a substitute contract. The fork wires
`device::get_logged_in_devices` to `/api/login/getdevicelist` and the visible
"Đăng xuất từ xa" action to `/api/login/kickdevice` (`gui/tools_panel.rs`,
`gui/worker.rs`).

This is **Unknown / AMBIGUOUS**, not proof that the endpoints do not exist:
dynamic server behavior or a different bundle path is possible. However, the
read-only listing and especially the remote logout mutation currently have no
verified caller, schema, or server evidence in the notes or the bundle scan.
Since the UI can issue a real session-termination request, fail closed and keep
these actions unavailable until an official caller/capture establishes the
contract. The related `account_recovery.rs` `/api/login/check2fa` reader is also
unverified by the current bundle scan, but it is read-only and should be tracked
separately. Unit parser tests do not raise these routes above E3.

The diagnostic redaction review was not promoted to a defect: although the
generic request logger can display bounded JSON previews in developer mode, no
captured plaintext response or bundle-backed response schema was found proving
that the candidate device/location/contact fields reach it. Broader key aliases
remain optional hardening, not an evidenced leak.

## Extended source pass: document preview privacy boundary

**P2 — Plaintext document preview survives privacy/account lifecycle changes.**
`GuiEvent::DocPreviewLoaded` retains only `file_name` and `content`
(`gui.rs:167–170`); its consumer at `2920–2923` accepts it without owner, runtime,
thread, hidden-state or expiry checks. The worker checks `media_context_alive`
before enqueueing (`gui/worker.rs:2144–2164`), which cannot guarantee state at GUI
consumption. More directly, an already opened `DocViewerModal` is not cleared by
`LoggedOut`, `ProfileSwitched`, or restrictive `apply_hidden_view`; these paths
clear `image_viewer` but omit `doc_viewer`. The viewer itself has no identity or
expiry guard. Thus a previously opened hidden-chat text preview can remain
resident and visible after relock or profile switch. A queued stale preview may
also arrive after a privacy-state change. Logout replaces the event channel,
which limits some late-event cases but does not remove the already opened viewer.

Correction should retain owner/runtime/message context in the event and viewer,
validate it at consumption/render, and clear the viewer at the same lifecycle
boundaries as other decrypted previews. This is a local privacy boundary finding,
not a newly inferred Zalo server contract. Parent and parity reviewer have been
asked to cross-review; no native/live reproduction is claimed here.

Extended reads cover complete queue-catchup/cursor, message-model/lifecycle,
updater, crash-report, upload-events, attachment-cache, ParamCipher, safety/prekey
modules, and several account/privacy/content/backup/download/document GUI panels.
GUI handlers were traced for backup, hidden-chat, group-content and privacy
command/event payloads. Independent parity reviewer added full presence, unread,
labels and avatar-history reads. Exact ranges are in the ledger; many larger
modules still require complete semantic review. No project-wide acceptance is
implied by this extension.
