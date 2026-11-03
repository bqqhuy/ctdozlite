# ADR-0008: Fail-closed mutations and coalesced background unread delivery

- **Status:** accepted
- **Date:** 2026-11-02
- **Deciders:** maintainer request to fix the eight remaining product audit findings

## Context and evidence

The audit of product c45882a, rechecked on bb434d0, reproduced UTF-8 parser panics, uncertain quote delivery replayed as SMS, empty upload responses accepted as success, media forwarding reduced to text, fabricated quote identity fields and discarded read cursors. Hidden Windows viewports dropped live events; whole-file uploads had no admission or size bound. These failures are represented by synthetic loopback fixtures in `tests/audit_regressions.rs`; no account or live mutation is used.

Existing protocol field evidence remains the quote/reaction/delete/recall contracts documented next to the operations in `src/chat.rs`, derived from the PC bundle references there. WireMessageRef and the request client ID already preserve the identifiers under ADR-0006. This change uses those identities for the existing fields; it does not introduce an endpoint, cipher, signing parameter or speculative media-forward protocol.

## Decision

Require an explicit zero error_code and a decodable acknowledgement envelope before accepting a mutation. Sends additionally require a nonzero server message ID. Missing or opaque responses are uncertain outcomes, never implicit success. An explicit mutation acknowledgement may have null data, but an upload/send may not. Keep encrypted responses fail-closed when decoding fails. Do not replay quote/link mutations as a second SMS after transport or protocol failures. This narrows the fallback behavior mentioned in ADR-0006 because cross-endpoint deduplication is not established. Missing quote references fail before sending rather than fabricating IDs or silently changing message kind.

Preserve server/client/sender identities from WireMessageRef and client_message_id in quote and message actions; timestamps and display names are not substitutes. Retain the observed wire field representation (numeric qmsgCliId/cMsgID; string cliMsgIdUndo). Plain text and stickers retain verified forwarding paths. Disable other forwarding actions with a Vietnamese explanation until a content-preserving protocol is verified; do not report a label-only SMS as forwarded media.

Apply read cursor and mute adapters during ingestion, merge Thread.unread, and respect countUnread. Local foreground reading advances only the local cursor, without sending a server receipt. A per-worker coalescing mailbox retains one wake-up and twenty resident messages per changed Thread while retaining exact unread IDs. Full message history is committed before publishing. Metadata-only read changes survive both WebSocket and polling delivery. Old account mailboxes stay on the old UI event channel.

On Windows, a native tray message-loop timer updates a generic red unread badge independently of viewport painting. TrayIcon remains on its owner thread; workers only set atomic attention flags. The badge and optional system sound contain no names or content. Sound respects global notification settings and local/server mute preferences. Clear attention when the UI consumes read state or logs out. This provides tray attention, not a Windows toast integration.

Bound the current in-memory upload implementation to two active uploads and 64 MiB per original. Image decoding also keeps the existing 16 MiB input/dimension/allocation limits. Read/hash/decode/cache work runs off the async/UI threads. Oversized/busy uploads fail explicitly before allocating the original. This is a bounded implementation, not streaming of arbitrarily large files. Clipboard format inspection may run on the UI thread; opening, reading, decoding and saving the image runs in the worker. CF_HDROP uses the same bounded reader/decoder.

## Verification and limits

Regression tests exercise missing/string/opaque receipts, quote/link disconnection without a second mutation, reference identity, unsupported forwarding, Unicode comments, read cursor ingestion, silent messages, new-thread unread and coalescing while the UI is paused. A sparse oversized original is rejected without a body allocation or network request. Existing identity/history/recall/local-flag tests remain required. The manual queue benchmark records both CPU overhead and retained events/messages; it does not claim target-device speed.

Windows ARM64 native CI still must run tray lifecycle, DX12 screenshots and clipboard/device checks. A host-side cross-check of the tray source validates Windows types without linking or running the product. Manual verification: hide the window, receive a permitted test message, observe the unread tray mark, restore and verify the badge; replay the same message and apply a remote read cursor; test muted and disabled notifications. Also verify text/sticker forwarding, explicit unsupported media forwarding and quote/recall on a consenting test account. No live account operation is automated here.

## Risk and rollback

Stricter acknowledgements or missing legacy reference metadata may reject an operation previously shown as successful. The user must inspect the conversation before manually retrying an uncertain send. Larger files must use another client until a verified streaming path exists. There is no dependency, cipher or schema change; rollback remains schema-5-compatible. Do not restore ambiguous mutation retries or fabricated identities as a workaround for a server compatibility issue.
