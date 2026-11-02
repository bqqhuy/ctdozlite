# ADR-0009: Durable outgoing retry, streaming uploads and Windows popups

Status: accepted for implementation; Windows device verification pending.

## Context

Failed sends previously only restored composer text. Uploads read the whole original
into RAM and rejected originals over 64 MiB or more than two concurrent requests.
Windows settings controlled taskbar attention/tray badges but had no message popup.

## Decision

All GUI sends share a durable outgoing payload and client ID. A pending bubble is
persisted before network work, and an error retains the original payload in its
conversation. Manual retry reuses the client ID and increments the attempt. A
confirmed acknowledgement replaces the pending bubble and removes retry data.
Server echoes also reconcile the local identity. Active jobs are guarded against
concurrent duplicate retries. Interrupted jobs remain available after restart.
Transport errors can mean an acknowledgement was lost: the bubble warns users to
check the conversation before retrying. There is no automatic mutation replay or
claim of exactly-once delivery.

Originals use one open regular file, a streaming MD5 pass, and 1 MiB upload chunks.
Two semaphore permits bound active files; other selections wait using paths and
metadata. The Windows picker supports multiple files. Image dimensions use header
inspection; the image preview decoder retains its own resource limits. The photo
single-shot fallback streams the multipart body with an explicit content length.
File size and modification time are checked around reads and before final send;
manual retry rejects changed originals. Service-side size limits still apply.
Wire endpoints, parameters, chunk size and checksum format are preserved.

Windows uses Shell_NotifyIconW NIF_INFO against the existing native tray icon.
A small vendored tray-icon patch exposes this operation with the actual private
native ID, preserving both upstream licenses. Its native timer runs while egui is
minimized or hidden. No external notification process is launched. The notification
and sound settings control popup admission and NIIF_NOSOUND independently. Muted
conversations and replayed numeric server IDs do not generate popups. Popup clicks
open the app. Generic popup text avoids exposing message contents. Windows focus
assist / notification settings remain authoritative; these are shell notifications,
not a persistent WinRT notification history implementation.

## Validation

Offline fixtures cover failed-send/retry acknowledgement and stable client IDs,
interrupted-state recovery, duplicate retry guards, large sparse-file chunk reads,
file mutation detection, and popup replay/mute gating. Standard tests, Clippy and
headless GUI smoke runs are recorded in the trial report. Native tray APIs are
cross-checked against Windows ARM64 bindings. Delivery with a real Zalo account,
Windows popup appearance/sound and Windows ARM64 launch require device validation.
