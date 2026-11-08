# E2EE Chat Integration — WP16 Sprint 3-4

## Overview

Wire E2EE Signal Protocol (Double Ratchet) into ZaloLite's send/receive message flow.

## Send Path

### Entry Point
- `src/gui/worker.rs` — WorkerCmd::SendMessage
- Check conversation metadata: `chat::is_conversation_e2ee(convo)`
- If E2EE enabled → call `chat::send_e2ee_message(session, profile_id, to_id, text, client_id)`
- Otherwise → original `chat::send_message_with_client_id()`

### E2EE Send Flow
```
plaintext → E2eeManager::encrypt() → wire format (PreKeyWhisper/Whisper)
         → base64 encode
         → POST {chat}/api/e2ee/pc/message/sms
           body: { toid, message: base64(ciphertext), clientId, isE2EE: 1 }
```

### First Message (Session Initiation)
1. Check `E2eeManager::load_session(peer_id)` → None
2. Fetch `prekey/get` → RemoteBundle (identity_key, signed_prekey, signature, one_time_prekey)
3. Upload our bundle via `prekey/update` (if not already done)
4. `E2eeManager::start_session(peer_id, bundle, plaintext)` → PreKeyWhisperMessage (type 3)

### Follow-up Messages
1. `E2eeManager::load_session(peer_id)` → Some(session)
2. `E2eeManager::encrypt(peer_id, None, plaintext)` → WhisperMessage (type 1)

## Receive Path

### Entry Point
- `src/chat.rs::parse_chat_message(it: &Value)`
- NEW: Check `e2eeStatus ∈ [3000, 4000)` → E2EE message
- Call `chat::try_decrypt_e2ee_content(profile_id, peer_id, raw)` → (plaintext, identity_changed)

### E2EE Receive Flow
```
WebSocket/HTTP polling → parse_chat_message
                       → detect e2eeStatus
                       → base64 decode content
                       → E2eeManager::decrypt(peer_id, payload)
                       → (plaintext, identity_changed)
```

### Error Handling
- **-29 (E2EE_DECRYPT_FAILED)**: MAC verification failed / session mismatch → show "[Tin nhắn mã hoá - không giải mã được]"
- **-30 (E2EE_PENDING_DATA)**: Missing prekey/session → fetch bundle, retry
- **identity_changed = true**: Peer's device reset → show warning banner in UI

## Storage

### Per-Profile Isolation
```
<root>/profiles/<name>/
  ├── messages.db       # Conversation history
  └── e2ee.db           # E2EE keys (identity, prekeys, sessions, trusted identities)
```

### E2EE Database Schema (e2ee.db)
See `src/crypto/store.rs` — 5 tables:
- `identity_key` — our identity (Curve25519 DH + Ed25519 signing)
- `signed_prekey` — rotated every 7 days
- `one_time_prekey` — refilled when count < 2
- `signal_session` — ratchet state per peer
- `trusted_identity` — TOFU tracking

## Testing

### Unit Tests
- `src/crypto/session.rs::tests` — 31 tests pass
  - `first_message_establishes_session_and_decrypts`
  - `follow_up_messages_use_the_stored_session`
  - `tampered_ciphertext_is_rejected`
  - `safety_number_is_stable_for_a_peer`

### Integration Tests (Manual)
1. **Handshake**: Alice → Bob first message → Bob receives + session established
2. **Roundtrip**: Alice ↔ Bob exchange 10 messages → all decrypt correctly
3. **Device Change**: Bob reinstalls → Alice sees ⚠️ warning + new safety number

## Security Properties

### Forward Secrecy
- Old message keys deleted after use → past messages safe if device compromised

### TOFU (Trust On First Use)
- First identity sighting → trusted automatically
- Subsequent mismatch → flag `identity_changed=true` → user verification required

### Key Isolation
- Private keys encrypted with DPAPI (Windows) before SQLite storage
- E2EE database separate from message history
