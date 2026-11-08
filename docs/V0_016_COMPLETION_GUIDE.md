# v0.016 Completion Guide — E2EE Integration & Manual Testing

**Target Audience:** AI agents or developers continuing v0.016 implementation.

**Current Status (2026-11-08):** Infrastructure 70% complete; needs protocol verification and UI indicators.

---

## 🔒 MANDATORY SECURITY CONSTRAINT

**Olaz/libsignal-protocol-javascript is GPL-3.0 and used for reference/algorithm study only — NO CODE COPIED.**

All E2EE implementations in ZaloLite are written in **Rust from the Standard Signal Protocol specification**. Never copy code from:
- `libsignal-protocol-javascript` (GPL-3.0)
- Olaz Swift implementations (GPL-3.0)
- Any GPL-licensed Signal implementation

**Reference documents** (algorithm study only):
- [re-notes/36-zalo-e2ee-signal-protocol-analysis.md](../re-notes/36-zalo-e2ee-signal-protocol-analysis.md)
- [re-notes/38-e2ee-chat-integration.md](../re-notes/38-e2ee-chat-integration.md)
- Standard Signal Protocol specification (public domain)

---

## 📊 WP16 Work Package Status

| WP | Component | Code Status | Verification Status | Blocker |
|---|---|---|---|---|
| **WP16-1** | PreKey Lifecycle | ✅ Implemented | ⚠️ Wire format unverified | Need live protocol capture |
| **WP16-2** | Double Ratchet | ✅ Implemented | ⚠️ Interop unverified | Need cross-client test |
| **WP16-3** | Multi-Account | ✅ Complete | ✅ Verified | None |
| **WP16-4** | Windows Hello | ✅ Complete | ✅ Verified | None |

**Summary:** Core crypto engine works (31 unit tests pass), but **protocol compatibility with official Zalo client is unverified**. Need manual device testing before production use.

---

## 🏗️ Architecture Overview

### File Structure

```
src/
├── crypto.rs                 # AES/MD5 primitives (login/transport)
├── crypto/
│   ├── keystore.rs          # IdentityKey, PreKey generation
│   ├── prekey.rs            # Server API: fetch/update prekey bundles
│   ├── ratchet.rs           # Double Ratchet state machine (613 LOC)
│   ├── safety.rs            # Safety number (60-digit fingerprint)
│   ├── session.rs           # E2eeManager orchestration (455 LOC)
│   ├── store.rs             # SQLite persistence (5 tables)
│   └── wire.rs              # Message serialization (PreKey/Whisper)
├── e2ee_integration.rs       # Bridge: crypto ↔ chat.rs send/receive
├── chat.rs                   # Send/receive wiring (lines 5812-5820, 8288-8345)
└── gui/worker.rs             # WorkerCmd::SendMessage E2EE check

re-notes/
├── 36-zalo-e2ee-signal-protocol-analysis.md  # Protocol reference
└── 38-e2ee-chat-integration.md                # Integration blueprint
```

### Data Flow

**Outgoing (first message):**
```
User types message → worker checks isE2EE flag → e2ee_integration::encrypt_message()
→ fetch peer's prekey bundle → X3DH handshake → RatchetSession::init_as_sender()
→ PreKeyWhisperMessage (type 3) → base64 → POST /api/e2ee/pc/message/sms
```

**Outgoing (follow-up):**
```
Load stored session → RatchetSession::encrypt() → WhisperMessage (type 1)
→ base64 → POST /api/e2ee/pc/message/sms
```

**Incoming:**
```
WebSocket/HTTP → parse_chat_message() → detect e2eeStatus ∈ [3000,4000)
→ base64 decode → e2ee_integration::decrypt_message() → RatchetSession::decrypt()
→ plaintext → store in SQLite → render in UI
```

---

## ⚠️ Known Gaps & Risks

### 1. Wire Format Unverified (CRITICAL)

**Issue:** [prekey.rs:7](../src/crypto/prekey.rs) states request/response schema is **"inferred, not captured live"**.

**Risk:** Official Zalo client may reject our prekey bundle or send bundles we can't parse.

**What's needed:**
- Capture live `/api/e2ee/pc/prekey/update` request from official client (Zalo PC 26.10.20+)
- Capture `/api/e2ee/pc/prekey/get` response
- Document exact JSON schema, key encoding (base64? hex?), signature format
- Verify `device_id` field (hardcoded to 1 in our code — correct?)

**Files to update:**
- [src/crypto/prekey.rs](../src/crypto/prekey.rs) — `fetch_bundle()`, `update_prekeys()`
- [src/crypto/keystore.rs](../src/crypto/keystore.rs) — `PreKeyBundle` struct

**How to capture:**
1. Run official Zalo PC with Charles Proxy / mitmproxy
2. Enable E2EE for a 1-1 conversation
3. Send first message → triggers prekey upload
4. **Redact secrets** (private keys, session tokens) before saving logs
5. Save sanitized request/response to `re-notes/fixtures/e2ee-prekey-*.json`

---

### 2. Ciphertext Container Format (CRITICAL)

**Issue:** [wire.rs:3](../src/crypto/wire.rs) uses **"custom format, not confirmed against Zalo"**.

**Risk:** Zalo server or recipient may reject our ciphertext, or we can't decrypt theirs.

**Current format (our implementation):**
```rust
// PreKeyWhisperMessage (type 3)
[version: u8] [type: u8] [bundle_len: u16 LE] [bundle_bytes] [header_len: u16 LE] [header] [ciphertext]

// WhisperMessage (type 1)
[version: u8] [type: u8] [header_len: u16 LE] [header] [ciphertext]
```

**What's needed:**
- Capture binary ciphertext from official client (base64 decode from network traffic)
- Reverse-engineer actual container format
- Check if Zalo uses libsignal-protocol-javascript's default serialization or custom wrapper

**Files to update:**
- [src/crypto/wire.rs](../src/crypto/wire.rs) — `serialize_prekey()`, `serialize_whisper()`, `deserialize()`

---

### 3. Signature Scheme Assumption

**Issue:** [session.rs:330](../src/crypto/session.rs) `verify_signed_prekey()` **assumes Ed25519**, but falls back to "not verifiable" instead of failing.

**Risk:** If Zalo uses different signature scheme (ECDSA P-256?), we silently skip verification → MITM vulnerability.

**What's needed:**
- Confirm signature algorithm from live capture
- Update verification or fail loudly if unsupported

---

### 4. Downgrade Attack Vector (SECURITY BUG)

**Location:** [chat.rs:8306](../src/chat.rs) (hypothetical — check actual `send_e2ee_message` error handling)

**Issue:** If E2EE encryption fails, **do not silently fall back to plaintext**.

**Fix required:**
```rust
// BAD (current pattern in some flows):
let ciphertext = encrypt_message(...).unwrap_or_else(|_| {
    // ❌ WRONG: Sends plaintext if E2EE fails
    text.as_bytes().to_vec()
});

// GOOD:
let ciphertext = encrypt_message(...)?;  // Propagate error, abort send
```

**Action:** Audit all E2EE send paths — **E2EE-required conversations MUST NOT send plaintext on encryption failure**.

---

### 5. Missing UI Indicators

**Roadmap requirement:** E2EE UI indicators (WP16 verification criteria).

**Not yet implemented:**
1. **Safety number dialog** — Compare 60-digit fingerprints with peer
2. **Identity warning banner** — Show when `identity_changed=true` (peer reinstalled)
3. **E2EE badge/icon** — Indicate message is encrypted in chat bubble

**Where to add:**

#### Safety Number Dialog
- **Trigger:** User clicks "View Safety Number" button in conversation settings
- **Logic:** Call `E2eeManager::safety_number(&peer_identity)` → format as 12 groups of 5 digits
- **File:** [src/gui/modals.rs](../src/gui/modals.rs) — new `SafetyNumberModal` struct
- **Reference:** [re-notes/36-zalo-e2ee-signal-protocol-analysis.md:76](../re-notes/36-zalo-e2ee-signal-protocol-analysis.md)

```rust
// Example implementation sketch:
pub struct SafetyNumberModal {
    pub peer_name: String,
    pub peer_id: String,
    pub safety_number: String,  // "01234 56789 01234 ..." (12 groups)
}

impl SafetyNumberModal {
    pub fn render(&mut self, ctx: &egui::Context, pal: &ThemePalette) {
        egui::Window::new("Số an toàn E2EE")
            .show(ctx, |ui| {
                ui.label(format!("Cuộc trò chuyện với {}", self.peer_name));
                ui.separator();
                ui.label(egui::RichText::new(&self.safety_number)
                    .font(egui::FontId::monospace(16.0))
                    .color(pal.text_primary));
                ui.label("Xác nhận số này trùng khớp trên cả hai thiết bị.");
                // TODO: Add QR code display option
            });
    }
}
```

#### Identity Warning Banner
- **Trigger:** `DecryptedMessage::identity_changed == true` in receive path
- **Display:** Yellow banner above chat: "⚠️ <Peer> đã đổi thiết bị. Xác minh lại số an toàn."
- **File:** [src/gui/messages.rs](../src/gui/messages.rs) — render banner before message list
- **Storage:** Track dismissed warnings in `messages.db` — don't spam on every message

#### E2EE Message Badge
- **Display:** Small lock icon 🔒 next to encrypted message bubbles
- **Logic:** Check `Message::is_e2ee` field (already exists at [chat.rs:87](../src/chat.rs))
- **File:** [src/gui/messages.rs](../src/gui/messages.rs) — render icon in message bubble

---

## 🧪 Manual Testing Checklist

### Prerequisites
- Two test accounts with E2EE enabled (check `isE2EE` flag in conversation metadata)
- One account on ZaloLite fork, one on official Zalo PC/mobile
- Charles Proxy / Wireshark to capture protocol (optional but recommended)

### Test Suite

#### T1: Initial Session Setup
1. **Fork → Official:** Send first E2EE message from ZaloLite
2. **Verify:** Official client receives and decrypts correctly
3. **Capture:** Save prekey bundle request/response (redact secrets)
4. **Check:** No error codes (-29, -30) in official client

#### T2: Roundtrip Communication
1. **Official → Fork:** Reply from official client
2. **Fork → Official:** Send 5 more messages
3. **Verify:** All messages decrypt correctly on both sides
4. **Check:** Message keys advance (no key reuse)

#### T3: Device Reset (Identity Change)
1. **Fork:** Delete `e2ee.db`, restart app (simulates reinstall)
2. **Fork → Official:** Send new message (triggers new X3DH)
3. **Verify:** Official client shows identity warning banner
4. **Check:** `identity_changed=true` in fork's receive path

#### T4: Out-of-Order Messages
1. **Official:** Send messages A, B, C rapidly
2. **Fork:** Receive in order B, A, C (simulate network reorder)
3. **Verify:** All decrypt correctly (skipped message keys work)

#### T5: Offline → Online
1. **Fork:** Go offline (airplane mode)
2. **Official:** Send 3 messages
3. **Fork:** Go online, fetch messages via HTTP polling
4. **Verify:** All messages decrypt correctly

#### T6: Group E2EE (if supported)
1. **Create group:** 3 members (fork + 2 official clients)
2. **All send messages:** Verify everyone receives plaintext
3. **Check:** Group E2EE uses Sender Key pattern (not pairwise encryption)

#### T7: Media Files
1. **Fork → Official:** Send encrypted photo
2. **Verify:** Official client downloads and decrypts image
3. **Check:** Media CDN URL is E2EE-wrapped or separately encrypted

#### T8: Error Handling
1. **Tamper ciphertext:** Flip random byte in captured payload
2. **Send to fork:** Verify MAC verification fails gracefully
3. **UI shows:** "Tin nhắn mã hoá - không giải mã được" (not crash)

---

## 🔧 Debugging Tools

### Inspect E2EE Database
```bash
sqlite3 ~/.config/zalolite/profiles/default/e2ee.db
.schema
SELECT * FROM identity_key;
SELECT key_id, created_at FROM signed_prekey;
SELECT COUNT(*) FROM one_time_prekey;
SELECT peer_id, updated_at FROM signal_session;
```

### Dump Safety Number
```bash
cargo test --package ctdozlite --lib crypto::session::tests::safety_number_is_stable_for_a_peer -- --nocapture
```

### Verify Key Rotation
```rust
let mgr = E2eeManager::open("test_profile")?;
let spk = mgr.signed_prekey()?;
let age_days = (now_ms() - spk.created_at) / 86_400_000;
println!("Signed prekey age: {} days (rotate at 7)", age_days);
```

---

## 📝 Implementation Checklist

### Phase 1: Protocol Verification (Est. 2-3 days)
- [ ] Capture live prekey upload request from official Zalo PC
- [ ] Capture live prekey fetch response
- [ ] Document exact JSON schema in `re-notes/fixtures/`
- [ ] Update [prekey.rs](../src/crypto/prekey.rs) to match official format
- [ ] Capture ciphertext container format (base64 decode network traffic)
- [ ] Update [wire.rs](../src/crypto/wire.rs) serialization
- [ ] Verify signature scheme (Ed25519 vs ECDSA)
- [ ] Run T1-T2 tests (initial handshake + roundtrip)

### Phase 2: Robustness (Est. 1-2 days)
- [ ] Add transaction wrapping to ratchet state updates
- [ ] Serialize session operations per peer (prevent race conditions)
- [ ] Audit downgrade attack vector — fail loudly on E2EE encryption error
- [ ] Handle missing prekey gracefully (refill + retry)
- [ ] Add retry logic with exponential backoff
- [ ] Run T3-T5 tests (device reset, out-of-order, offline)

### Phase 3: UI Indicators (Est. 1-2 days)
- [ ] Implement SafetyNumberModal in [modals.rs](../src/gui/modals.rs)
- [ ] Add identity warning banner in [messages.rs](../src/gui/messages.rs)
- [ ] Add E2EE badge/icon to encrypted message bubbles
- [ ] Wire "View Safety Number" button in conversation settings
- [ ] Add dismissible warning storage to `messages.db`

### Phase 4: Manual Testing (Est. 2-3 days)
- [ ] Run full T1-T8 test suite
- [ ] Test with real accounts (not test/dev accounts)
- [ ] Verify no secrets leaked in logs
- [ ] Check server-side ciphertext is opaque (base64 blob, no metadata)
- [ ] Confirm forward secrecy (delete old message keys)

### Phase 5: Documentation (Est. 1 day)
- [ ] Update [ROADMAP_PHASE_2.md](../docs/ROADMAP_PHASE_2.md) with verified status
- [ ] Write `re-notes/39-e2ee-verification-results.md` with test outcomes
- [ ] Document any protocol deviations from standard Signal
- [ ] Add troubleshooting guide for common E2EE failures

---

## 🚨 Gotchas & Pitfalls

### 1. DPAPI Encryption on Windows
- Private keys stored in SQLite are encrypted with Windows DPAPI
- **Do not** commit `e2ee.db` to git (contains DPAPI-protected keys)
- Keys are user-scoped — copying DB to another Windows user won't work

### 2. Device ID Hardcoded to 1
- Zalo PC uses `device_id=1` for all sessions
- Multi-device support (desktop + mobile same account) **not implemented**
- Needs separate device registration flow (out of scope for v0.016)

### 3. Group E2EE Unverified
- Routes exist ([36-zalo-e2ee-signal-protocol-analysis.md:156](../re-notes/36-zalo-e2ee-signal-protocol-analysis.md))
- Mechanism inferred as Sender Key pattern, **not confirmed**
- Defer group E2EE to v0.017 if time-constrained

### 4. Message Type Codes
- Type 3 = PreKeyWhisperMessage (first message, includes bundle)
- Type 1 = WhisperMessage (follow-up messages)
- **Do not** mix up — sending type 1 before session exists crashes recipient

### 5. One-Time PreKey Consumption
- Server consumes one prekey per new session
- **Must** refill when count < 2 (see [session.rs:28](../src/crypto/session.rs))
- Failure to refill → new peers can't start sessions with you

---

## 📚 Reference Materials

### Core Files to Read First
1. [re-notes/36-zalo-e2ee-signal-protocol-analysis.md](../re-notes/36-zalo-e2ee-signal-protocol-analysis.md) — Protocol analysis
2. [src/crypto/session.rs](../src/crypto/session.rs) — E2eeManager API
3. [src/crypto/ratchet.rs](../src/crypto/ratchet.rs) — Double Ratchet implementation
4. [src/e2ee_integration.rs](../src/e2ee_integration.rs) — Integration bridge

### External References
- [Signal Protocol Specification](https://signal.org/docs/) — Canonical reference
- [X3DH Key Agreement](https://signal.org/docs/specifications/x3dh/) — Initial handshake
- [Double Ratchet Algorithm](https://signal.org/docs/specifications/doubleratchet/) — Session state machine

### Test Evidence
- Unit tests: `cargo test crypto::session::tests` (31 tests, all passing)
- Integration test stub: [src/crypto/session.rs:352](../src/crypto/session.rs) — expand with live data

---

## 🎯 Success Criteria (v0.016 Completion)

From [ROADMAP_PHASE_2.md:174](../docs/ROADMAP_PHASE_2.md):

> "Hai tài khoản thật bật E2EE trao đổi tin nhắn văn bản, ảnh và file: Cả hai giải mã đúng 100%, nội dung lưu trên server Zalo chỉ là bản mã nhị phân không thể đọc được."

**Translation:** Two real accounts with E2EE exchange text, images, files: both decrypt correctly 100%, server stores only opaque ciphertext.

**Verification:**
1. ✅ Text messages: T1-T2 tests pass
2. ✅ Images: T7 test passes
3. ✅ Files: T7 test passes (same as images)
4. ✅ Server ciphertext opaque: Inspect network traffic — no plaintext metadata
5. ✅ UI indicators: Safety number dialog + identity warning banner functional
6. ✅ Multi-account: WP16-3 already complete

---

## 🤝 Handoff Notes

**What works:**
- X3DH handshake (sender + receiver)
- Double Ratchet encryption/decryption
- SQLite persistence (5 tables, DPAPI-protected keys)
- Prekey generation + refill logic
- Safety number calculation
- Multi-account profile isolation (WP16-3)
- Windows Hello biometric lock (WP16-4)

**What needs verification:**
- Wire format compatibility with official Zalo client
- Signature scheme (Ed25519 assumption)
- Ciphertext container format
- Group E2EE mechanism

**What's missing:**
- Safety number dialog UI
- Identity warning banner UI
- E2EE message badge/icon
- Manual device testing (T1-T8)

**Estimated effort to complete:** 5-8 days for one developer/agent.

---

**Last Updated:** 2026-11-08  
**Author:** Claude Opus 5.5 (session e1b94e2b)  
**Next Agent:** Read this guide top-to-bottom before touching E2EE code. Start with Phase 1 (protocol verification).
