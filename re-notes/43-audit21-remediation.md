# Audit-21 remediation evidence and protocol boundaries

Baseline: `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`, 2026-11-16.
The maintainer authorized P1, then P2, then P3 implementation and a two-reviewer
audit before merge. This note records correctness decisions, not a new server contract.

## P1: local account and history correctness

Voice commands/results carry the originating Session runtime, owner and target;
account/thread transitions invalidate the draft. The WAV attachment transport
is unchanged and still does not claim native Zalo voice-codec parity. The TUI
uses the existing hidden-chat Vault/read/unlock API and ADR-0034 display policy.
Delete-for-me keeps the existing server request, then commits local tombstones
before success. Replays use direction/sender/provisional-ID semantics from the
existing store. Distinct server identities are never merged merely by client ID.

## P2: crypto verification

F13: the synthetic bundle's independent signing hint no longer substitutes for
the DH identity used by X3DH/trust. XEdDSA verification must use that DH identity;
unproven distinct-key binding and the independent Ed25519 fallback fail closed.
No inferred server field or endpoint is added. See ADR-0038.

F19: Signal display fingerprints bind each party's stable identifier and
DJB-serialized key. The per-party SHA-512 seed is display scheme version 0,
key and identifier; the key is included in every hash round, with 5200 rounds
total. Six 5-byte groups modulo 100000 form each party's 30 decimal digits;
lexically ordered party strings produce the 60-digit display. This is an
algorithm description independently implemented locally, not copied source code.
Public vector metadata and expected output are preserved in the audit artefacts.

Source read on 2026-11-16:
<https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/fingerprint.rs>.
This is primary Signal evidence, not E1/E2 evidence of Zalo's identifier choices.
The identifier-bound manager API uses account/peer IDs already in its context;
official Zalo compatibility remains Unknown — AMBIGUOUS. Old verification flags
are reset by crypto schema 9; key/session/receipt data are retained.

## P2: transport, parser and logging

F11 retains the decoded static PC framing/command list documented at the top of
`src/socket.rs`; only local write deadline/cancellation behavior changes. No
heartbeat bytes, command IDs or speculative mutation retry is introduced.

F18 uses the account context already supplied by the preview/WebSocket adapter
to recognize its own sender UID. Identity/merge boundaries are documented in
`re-notes/12_exact_message_identity_and_quotes.md`. Generalization to every phone
sync packet remains Unknown — AMBIGUOUS until captured fixtures establish that
packet shape. The separate phone-transfer adapter is preserved.

F12 is local diagnostic hygiene: raw QR/auth bodies, cookie prefixes and arbitrary
server error strings are removed from verbose/error paths. Status, byte counts,
numeric error code and cookie presence can be logged. Request payloads/signing
remain unchanged; sanitized synthetic sentinels are used in tests.

## P2: GUI and native resource policy

F09 moves capture/preview setup to a bounded device-owning thread, retains PCM
through immutable Arc snapshots and encodes WAV in a blocking worker. Recording
is limited locally to five minutes and 64 MiB of mono f32 samples. This is an
explicit fork resource budget, not an observed Zalo server/media limit. Reaching
the limit is surfaced to the user. Record/preview commands and results carry the
draft generation; out-of-band invalidation stops stale device work even when
the command queue is full. Idle device work waits for commands instead of polling.

F08 retains the existing reaction add/remove contract; a local intent/revision
prevents removal being dispatched as addition or stale completion overriding
new state. F10 separates trial/identity facts from unverified conversation
encryption. F20 retains a single account popover and its actual switch/add commands.

F21 follows Microsoft's desktop HWND interop guidance, not the UWP-only request
entry. The requesting window belongs to this process and is rechecked on the
native MTA thread. One native operation owns admission until its real completion;
caller timeout cannot free that lease and create unlimited blocked threads.
Request/runtime guards reject late completion; recovery never fabricates unlock.
Source: <https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier>.
This is OS API evidence, not native-device acceptance.

## Check results

Phase-1 check and strict Clippy passed; library suite: 786 passed, 0 failed,
10 ignored on Windows x64. Native microphone/TUI testing and Zalo interoperability
were not performed. Phase-2/3 results and reviewer decisions must be recorded
from actual commands in the implementation tracker and final handover, never
inferred from this note or the historical audit test counts.
