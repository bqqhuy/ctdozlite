# P0 baseline verification

Date: 2026-11-21

Status: **partial; no native behavior measurements recorded**

Scope: evidence check for the realtime/send P0 plan. This report contains no message content, destination identifiers, session data, cookies, URLs, or account database data.

## Baseline artifact and processes

- Existing artifact: `C:\Apps\ZaloLite\ctdozlite-0.016-arm-portable-windows-arm64.exe`
- File size: 43,304,960 bytes; last modified 2026-11-20 00:23:52 (local time).
- SHA-256: `21E42B668B3018965B3FBBB365C91210EBCC93A437D43D11053C722766274807`.
- PE/COFF machine field: `0xAA64` (ARM64).
- At inspection, that baseline executable was running as PID 21372. A second process, `zalo_rust-0336a315cd25802f`, was running from a Claude worktree under `.claude\worktrees`; it is not the installed baseline artifact.

The baseline was identified, not behaviorally exercised. No test messages or attachments were sent, and no receive/sync/media interaction was observed during this check. Therefore this report makes no claim about send latency, group policy loading, inbound delivery, calls, or UI behavior on the device. The installed binary predates the current working-tree changes and cannot validate them.

## Source inspection: instrumentation currently available

`src/service_resolver.rs` and `src/outgoing.rs` contain a bounded process-local send-phase ring (limit 256) with sanitized timing fields. It records phases such as queue wait, policy, HTTP request/attempt/body ACK, link preview, and total intent. The records omit message text, destination, URL, and raw client message ID; the client ID is reduced to a keyed tag. Existing timings are diagnostic samples, not an aggregated benchmark or an exported P0 report.

The current working-tree change in `src/outgoing.rs` records queue wait but starts the text-send deadline when the worker starts sending. This is a fork behavior change that still requires focused tests and native measurement; it does not provide evidence that end-to-end queue delay has improved.

`src/gui.rs::apply_outgoing_state` reconciles by `client_message_id`, updates the matching thread, refreshes sidebar presentation, and requests persistence for each outgoing-state event. `WorkerCmd::PersistThreads` persists a local thread snapshot and cache. The source currently has no measurements for Enter-to-Pending, worker dequeue-to-send start, event queue depth, SQLite transaction/busy time, full snapshot clone/cache write cost, ingress-frame-to-UI latency, or egui frame p95. Presence of timing hooks for send phases does not measure these missing quantities.

## Diff classification at inspection

The working tree had edits in `src/gui/worker.rs`, `src/gui/workspace.rs`, `src/outgoing.rs`, `src/service_resolver.rs`, `src/session.rs`, `src/storage.rs`, `docs/ZALO_REFERENCE_ANALYSIS.md`, and `re-notes/44-map15-network-send.md`, plus the realtime plan and research notes. They were uncommitted and are treated as in-progress evidence, not completed fixes.

- Keep in P1 review: outgoing owner/runtime admission and terminal result changes in `src/gui/worker.rs`; account-write coordination changes in `src/session.rs` / `src/storage.rs` are also in progress and need complete writer coverage and race tests.
- Keep for P2 evaluation: queue-wait no longer failing the send solely because the command sat in the queue (`src/outgoing.rs`, `src/service_resolver.rs`, `re-notes/44-map15-network-send.md`). It is not yet measured as a user-visible improvement.
- Preserve as unrelated to P0/P1 unless explicitly covered by the action plan: other workspace/UI changes and reference-analysis additions. Do not infer completion from their presence.

## Toolchain observation

The pinned Cargo executable was not on the initial process `PATH`, but was found at `%USERPROFILE%\.cargo\bin\cargo.exe`. `clang.exe` is present at `C:\Program Files\LLVM\bin\clang.exe`, and CMake is present at `C:\Program Files\CMake\bin\cmake.exe`. With Cargo, LLVM, and `CMAKE_POLICY_VERSION_MINIMUM=3.5` on the process environment, `cargo check --locked` passed `ring` compilation and stopped in `audiopus_sys`: its build script selected the Visual Studio 17 2022 generator, but the required Visual Studio C++ toolchain/`cl.exe` is not installed or discoverable. This is a fresh check result; it does not verify the crate or current changes.

## P0 exit criteria

P0 is **not complete**. The artifact/process baseline and safe source-level instrumentation inventory are recorded, but the repeatable device scenario and the required latency/queue/database/frame measurements have not been captured. A later device run should separately record Enter-to-Pending, dequeue-to-request-start, request-to-authoritative result, and inbound-frame-to-render timings, with workload, sample count, and p50/p95. Do not store message or session data in those measurements. Fixture DB tests can establish persistence timing separately but cannot substitute for device acceptance.

## Follow-up verification on 2026-11-21

The P1 implementation was checked separately from the installed baseline binary. On this Windows ARM64 workspace, the following checks ran against the current source tree:

- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed (Git emitted only line-ending conversion notices for existing edited files).
- `cargo check --tests --locked -j 1` — passed.
- `cargo test --locked --lib -j 1` — 999 passed, 0 failed, 10 ignored; the native H.264 test remains among device-dependent ignored tests.
- `cargo clippy --all-targets --locked -j 1 -- -D warnings` — passed.
- Two independent read-only P1 audits — both accepted the account-generation barriers, outgoing terminal outcomes, and session transition ordering.

These results verified compilation and unit behavior. The installed binary under `C:\Apps\ZaloLite` was not behaviorally exercised; the following native run used the newly built debug binary from `target/debug`.

## Native test build and UI audit on 2026-11-21

At the first read-only app inventory no Zalo window was returned, so I built the requested test executable and launched it:

- Command: `cargo build --locked --bin ctdozlite -j 1` — passed in 2m15s.
- Executable: `C:\Projects\ZaloLite\target\debug\ctdozlite.exe`, 88,227,840 bytes.
- SHA-256: `16953D5366D388628E08B8BB1D7678FA594BA72CAE569527EC31DE4579398294`.
- Device: Snapdragon 8cx Gen 3, 8 logical processors; restored the existing account session. The app remains open on My Cloud Document.
- Direct observations: a native “cannot save update to SQLite; keep old cursor” warning persisted; a ~96–108px black strip separated sidebar and chat at both normal and maximized sizes; contacts used initials while chat/sidebar used image/composite avatars; the sticker/emoji popup opened over the sidebar instead of anchoring near the composer; a Henry call card collapsed its text into a very narrow column.

### Controlled messages and observed timing

Two single messages sent to the user's own Cloud conversation reached the local timeline with **Sent** status. The UI tool returned the first screenshot 848ms after Enter and the second 1,014ms after Enter. These times include desktop automation and screenshot overhead, so they are not pure UI/transport timings and do not establish server ACK latency.

I attempted a five-message burst both with text injection and direct key events. The text-injection run merged/repeated input; the direct-key run rendered five intents as three bubbles with malformed/merged text. That is a reproducible failure of the combined UI/input path, but automation can synthesize keyboard events differently from a human keyboard. It requires a human-input reproduction before assigning the cause to the composer or worker queue. A Shift+Enter check also sent the current draft instead of inserting a newline in this run; the key-event path must be retested with a physical keyboard/IME. No messages were sent to Henry.

### Resource sample

With Cloud open and no intentional UI input for 10.009s, the process accumulated 11.016s CPU, or 110.06% of one logical processor / 13.76% of the 8-logical-processor total. Working set was 1,329.59 MiB and private memory 1,211.73 MiB. This was one sample from a debug process with a real restored account and uncontrolled background traffic; it is not a release benchmark and cannot establish a leak or production idle cost.

### Unverified flows and full visual evidence

Clicking the file picker did not expose a dialog in the window inventory; the image button showed “file chooser opening” but no chooser became observable. No attachment/image upload, call, or group send was completed. A resize drag did not change window size, so narrow-width responsiveness was not measured. There is no matched live Zalo PC capture for the same data, viewport, DPI, and theme.

Detailed independent visual findings and priority recommendations are recorded in [the native test build UI audit](2026-11-21-native-test-build-ui-audit.md). P0 remains partial: only two single-send responses, a malformed automated burst, one idle resource sample and visual issues were observed; ingress-to-render, ACK/echo, queue depth, SQLite timings and frame p95 are still unmeasured. These observations do not substitute for human keyboard, media, call, receive-during-sync, or release-build acceptance.
