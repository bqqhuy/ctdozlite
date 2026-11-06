# ADR-0034: Hidden-chat PIN compatibility and local display gate

Date: 2026-11-06. Status: accepted for the authorized v0.008 implementation.

PC 26.11.10 HiddenChatManager (~4798100–4813000) hashes a four-digit PIN with MD5 for
`hiddenconvers/update-pin`. `get-all` returns `pin` (protocol verifier) and `threads`
(`thread_id`, `is_group`). The server has no separate verify-PIN request in this flow.

Use MD5 only to interoperate with this existing protocol, not as a new password KDF,
integrity primitive, DB encryption or secure local password store. Compare verifiers
using aws-lc-rs's constant-time comparison. Keep verifier and entered PIN in zeroizing
memory; do not persist either or log/dump hidden-chat exchanges. No dependency added.
Serialize changes and refresh, validate readback, gate locked chats in UI/search/export,
and clear visible history/media when locking or receiving a remote PIN/list change.
Five failed unlocks cause a one-minute runtime lockout; a new app process resets it,
so this is a usability/privacy gate, not resistance against a local attacker.

SQLite and downloaded files remain unencrypted; PIN does not protect filesystem access.
Do not expose the destructive native reset workflow without a separate reviewed intent.
Device interoperability remains a native acceptance check, not proven by fixtures.

Persist only owner-bound hidden membership (no PIN/verifier) for offline privacy.
An offline restart permits ordinary chats but keeps hidden chats locked until a
fresh server verifier is available. Missing verifier is not evidence of no PIN;
do not enable unlock or PIN replacement from that incomplete response.
