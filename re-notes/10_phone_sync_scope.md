# Phone-history synchronization scope — 2026-11-02

## Evidence

- Official user documentation: https://help.zalo.me/tai-zalo/ explicitly lists synchronization between computer and phone. It does not document a third-party protocol or prove every old message/media item is recoverable.
- `re-notes/02_chat_data.md`, observed from the shipped Zalo PC 26.10.11 bundle: full conversation/history synchronization uses dedicated sync-v2 workers and binary framing, distinct from the REST preload path.
- Current fork implements verified polling/preload pieces and the CM group/Cloud history endpoint. `src/history_sync.rs` is bounded REST pagination, not a complete phone-to-desktop history transfer implementation.

## Unknowns before implementation

Identify the current request/approval flow, device/session binding, required key exchange and encrypted framing, transfer/download lifecycle, supported retention and account-scoped import behavior. Inspect shipped code and sanitized traffic only from an explicitly authorized test account/device. Do not reuse embedded test private keys from historical notes as real session/device identity.

No fresh phone capture, login, message transfer or backup import was performed here. These gaps establish missing implementation/verification, not technical impossibility or a guarantee that a future implementation will recover all old data.

## UI language

Use “Phone history synchronization is not yet supported in this build,” and distinguish recent conversation refresh from group/Cloud history download. Keep actual progress visible on the main workspace. Avoid statements that security alone prevents synchronization or that downloading zero records means the phone history has been synchronized.
