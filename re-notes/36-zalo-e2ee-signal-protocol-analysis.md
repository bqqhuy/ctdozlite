# RE-NOTE 36: Zalo E2EE — Signal Protocol Analysis

Source: `zalo-swift-re` (Olaz) — GPL-3.0, for reference only; no code copied.
Date: 07/11/2026. Analyst: Claude Opus 5.5.

---

## 1. Nguồn bằng chứng

- `ZaloSetup-universal-26.10.20.dmg → app.asar → pc-dist/libs/libsignal-protocol.static.js`
  → vendored verbatim vào Olaz tại `Sources/Olaz/JSModules/vendor/libsignal-protocol.js` (11 LOC minified, ~615KB)
- Upstream: `signalapp/libsignal-protocol-javascript` (GPL-3.0)
- VNG fork additions (confirmed from comments): `getSessionTs`, per-cipher logger/listener options, Node/$znode hooks

---

## 2. Wire Protocol Zalo E2EE

### 2.1 Nhận dạng E2EE message (server-side field)

```
message.e2eeStatus >= 3000 && message.e2eeStatus < 4000  →  isE2EE = true
```

- Nguồn: `chat-adapter.js:750`
- Nếu `e2eeStatus` ngoài range 3000-3999: tin nhắn thường (hoặc draft/pending)

### 2.2 Loại message synthetic khi E2EE gặp lỗi

```js
MSG_MORE.E2EE_DECRYPT_FAILED  = -29   // "Giải mã tin nhắn không thành công."
MSG_MORE.E2EE_PENDING_DATA    = -30   // "Đang chờ tin nhắn này"
```

- Nguồn: `chat-adapter.js:281`
- `-29` xuất hiện khi session chưa thiết lập hoặc key bị mất
- `-30` xuất hiện khi device chưa nhận được PreKey bundle của sender

### 2.3 Send path

Olaz delegate hoàn toàn cho official `app.asar` renderer thông qua:
```js
cc._processSendMessage(MSG.TEXT, String(text), list, quoted, true, info, true, null, null);
```

- Tức là **không có wire format mới** — E2EE được xử lý bởi official renderer JS
- Payload gửi lên server qua `/api/e2ee/pc/message/sms` (1-1) và `/api/e2ee/pc/group/sms` (group)
- Format cụ thể của ciphertext body **KHÔNG bộc lộ qua Olaz** vì renderer thực hiện ngầm

---

## 3. Signal Protocol Implementation

### 3.1 Standard Signal Protocol (không thay đổi so với upstream)

Zalo dùng Signal Protocol chuẩn — **KHÔNG thay đổi thuật toán core**:

```
X3DH Handshake (Initial session setup):
  Alice → Bob:
    1. Fetch Bob's PreKey bundle từ server:
       {identityKey, registrationId, signedPreKey{keyId, publicKey, signature}, preKey{keyId, publicKey}}
    2. SessionBuilder.processPreKey(bundle)
       → X3DH: DH(IK_A, SPK_B) || DH(EK_A, IK_B) || DH(EK_A, SPK_B) || DH(EK_A, OPK_B)
       → derive RootKey + SendChainKey
    3. First message type = 3 (PreKeyWhisperMessage)
       {registrationId, preKeyId, signedPreKeyId, baseKey (EK_A), identityKey (IK_A), message}

Double Ratchet (Ongoing):
  - Subsequent messages: type = 1 (WhisperMessage)
  - Each send: advance SendChain KDF → MessageKey → AES-256-CBC + HMAC-SHA256
  - On receive new ratchet key: DH ratchet → new RootKey, SendChainKey, RecvChainKey

Safety Number:
  - FingerprintGenerator.createFor(a, pubKey_a, b, pubKey_b)
  - FINGERPRINT_ITERATIONS = 5200
  - 60 digits, grouped 5s, symmetric (a↔b same result)
```

### 3.2 VNG-specific additions

Dựa trên comment trong libsignal-protocol.js header:

| Addition | Ý nghĩa |
|---|---|
| `getSessionTs()` | Lấy timestamp của lần reset session gần nhất cho một peer. Zalo dùng để phát hiện "session reset" và hiển thị thông báo "Thiết bị mới của X bắt đầu cuộc trò chuyện" |
| Per-cipher logger options | Debug logging hook, không ảnh hưởng crypto |
| `$znode` hooks | Thay thế Node.js built-ins khi chạy trong Electron/JSC environment |

### 3.3 Key store interface (Zalo MemoryStore)

```js
// Standard libsignal-protocol interface — persistent impl cần thay thế MemoryStore
class MemoryStore {
  // Identity keys
  isTrustedIdentity(identifier, identityKey)  // Trust-on-first-use + pin
  saveIdentity(identifier, identityKey)        // Returns true nếu key changed (device reset)
  
  // PreKeys (one-time, consumed per session setup)
  loadPreKey(keyId), storePreKey, removePreKey
  
  // Signed PreKeys (rotate ~7 days)
  loadSignedPreKey(keyId), storeSignedPreKey, removeSignedPreKey
  
  // Session records (per peer address)
  loadSession(identifier), storeSession, removeSession
  
  // Zalo addition:
  getSessionTs(name)  // → timestamp | undefined
}
```

### 3.4 Address format

```js
const address = (id) => new signal.SignalProtocolAddress(id, DEVICE_ID)
// DEVICE_ID = 1 (Zalo PC luôn dùng device_id=1)
// identifier format: userId string (UUID hoặc numeric Zalo UID)
```

---

## 4. Key Lifecycle

### 4.1 Batch sizes (từ zalo-e2ee.js)

```js
PREKEY_BATCH    = 5    // onetime prekeys generate per batch (server có thể yêu cầu nhiều hơn)
SIGNED_PREKEY_ID = 1   // ID cố định (demo) — production cần rotation
```

### 4.2 Registration flow

```
1. KeyHelper.generateRegistrationId()  → registrationId (random u32)
2. KeyHelper.generateIdentityKeyPair() → {pubKey, privKey} (Curve25519)
3. KeyHelper.generateSignedPreKey(identity, SIGNED_PREKEY_ID)
   → {keyId, keyPair, signature}  (signature = Ed25519 sign of Curve25519 pubKey)
4. generatePreKey(keyId++) × N  → one-time prekeys
5. POST /api/e2ee/pc/prekey/update  với bundle:
   {identityKey, registrationId, signedPreKey{keyId, publicKey, signature}, preKeys[{keyId, publicKey}]}
```

### 4.3 PreKey bundle consumption

- Server trả về bundle khi client A muốn chat E2EE với client B
- Mỗi bundle dùng **1 one-time prekey** (consumed, không tái dụng — perfect forward secrecy)
- Khi server hết one-time prekeys: vẫn dùng signed prekey only (giảm security)
- Client phải refill proactively khi còn ít keys

---

## 5. Group E2EE

### 5.1 Routes đã biết (từ re-notes/fixtures/v008-official-route-index.json)

```
/api/e2ee/pc/group/sms        text message
/api/e2ee/pc/group/photo      photo
/api/e2ee/pc/group/file       file
/api/e2ee/pc/group/gif        GIF
/api/e2ee/pc/group/sticker    sticker
/api/e2ee/pc/group/quote      quoted reply
/api/e2ee/pc/group/mention    mention
/api/e2ee/pc/group/forward    forward
/api/e2ee/pc/group/undoMsg    recall
/api/e2ee/pc/group/multiUndoMsg  batch recall
/api/e2ee/pc/group/deleteMsg  delete
/api/e2ee/pc/group/deleteConv delete conversation
/api/e2ee/pc/group/contact    contact card
/api/e2ee/pc/group/link       link preview
```

### 5.2 Group E2EE mechanism (inferred)

Signal Protocol chuẩn không hỗ trợ native group E2EE với key rotation per-member.
Zalo khả năng cao dùng **Sender Key** pattern (Signal Groups):

```
Group setup:
  1. Group creator generates SenderKey (random chain key + signature key)
  2. Distribute SenderKey to each member individually via 1-1 session
  3. Each member sends group messages encrypted with their own SenderKey
  4. Recipients dùng stored SenderKey của sender để decrypt

Member join/leave:
  - New member: existing members re-distribute SenderKey OR generate new one
  - Member leaves: generate new SenderKey, redistribute (break-in security)
```

**Chưa xác minh được** vì Olaz không expose group E2EE wire format.

---

## 6. WebSocket endpoint: `wsN-msg.chat.zalo.me`

Từ `ZaloMainProcess.swift:464`:
```swift
// "subdomains such as `ws5-msg.chat.zalo.me`"
// Foundation sets cookie with leading dot for domain cookies
```

**Phát hiện quan trọng**: Olaz confirm Zalo có **ít nhất 2 WebSocket cluster**:
- `zpw_ws` (hiện ZaloLite dùng): control socket, push transport, CMD 501/502/521...
- `wsN-msg.chat.zalo.me` (N = 1..N): message-specific WebSocket, có thể là channel riêng

→ **Cần probe**: ZaloLite có thể đang bỏ sót message push channel thứ hai.

---

## 7. Kết luận & Action Items cho ZaloLite v0.016

### 7.1 Crypto crates cần thêm vào Cargo.toml

```toml
# E2EE Signal Protocol
x25519-dalek = { version = "2", features = ["static_secrets"] }
ed25519-dalek = { version = "2", features = ["rand_core"] }
hkdf = "0.12"
hmac = "0.12"
sha2 = "0.10"   # đã có
# aws-lc-rs đã có — dùng cho CSPRNG thay rand
```

`aes-gcm` đã có trong Cargo.toml. `aws-lc-rs` đã có (CSPRNG).

### 7.2 Module structure cho src/crypto/

```
src/crypto.rs          (existing — transport AES, MD5)
src/crypto/ratchet.rs  (NEW — Double Ratchet state machine)
src/crypto/store.rs    (NEW — SQLite-backed SignalProtocolStore)
src/crypto/keystore.rs (NEW — identity + prekey lifecycle)
src/crypto/prekey.rs   (NEW — server API for prekey upload/fetch)
```

### 7.3 SQLite schema cần thêm vào store.rs

```sql
-- Identity keys (one row per account profile)
CREATE TABLE identity_key (
    profile_id TEXT NOT NULL,
    public_key BLOB NOT NULL,    -- 32 bytes Curve25519
    private_key BLOB NOT NULL,   -- 32 bytes, DPAPI-encrypted on Windows
    registration_id INTEGER NOT NULL,
    PRIMARY KEY (profile_id)
);

-- Signed prekeys (rotate ~7 days, keep last N for in-flight decryption)
CREATE TABLE signed_prekey (
    profile_id TEXT NOT NULL,
    key_id INTEGER NOT NULL,
    public_key BLOB NOT NULL,
    private_key BLOB NOT NULL,   -- DPAPI-encrypted
    signature BLOB NOT NULL,     -- 64 bytes Ed25519
    created_at INTEGER NOT NULL,
    PRIMARY KEY (profile_id, key_id)
);

-- One-time prekeys (consumed on session setup)
CREATE TABLE one_time_prekey (
    profile_id TEXT NOT NULL,
    key_id INTEGER NOT NULL,
    public_key BLOB NOT NULL,
    private_key BLOB NOT NULL,   -- DPAPI-encrypted
    PRIMARY KEY (profile_id, key_id)
);

-- Session records (per peer, serialized Double Ratchet state)
CREATE TABLE signal_session (
    profile_id TEXT NOT NULL,
    peer_id TEXT NOT NULL,
    device_id INTEGER NOT NULL DEFAULT 1,
    session_record BLOB NOT NULL,  -- serialized ratchet state, DPAPI-encrypted
    session_ts INTEGER,            -- Zalo addition: last reset timestamp
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (profile_id, peer_id, device_id)
);

-- Trusted identity keys (TOFU: trust-on-first-use)
CREATE TABLE trusted_identity (
    profile_id TEXT NOT NULL,
    peer_id TEXT NOT NULL,
    identity_key BLOB NOT NULL,   -- 32 bytes
    PRIMARY KEY (profile_id, peer_id)
);
```

### 7.4 Verification test plan (port từ E2EETests.swift)

```rust
#[test]
fn test_roundtrip_establishes_session_then_ratchets() {
    // Alice sends first message → type 3 (PreKeyWhisperMessage)
    // Bob replies → type 1 (WhisperMessage, ratchet advanced)
    // 5 more messages → all type 1, each unique MessageKey
}

#[test]
fn test_tampered_ciphertext_rejected() {
    // Flip bit in ciphertext body → decrypt must return Err
}

#[test]
fn test_safety_number_symmetric_60_digits() {
    // safetyNumber(a, b) == safetyNumber(b, a)
    // Result is 12 groups of 5 digits
}

#[test]
fn test_e2ee_status_range() {
    // Parse server message: e2eeStatus in [3000, 4000) → isE2EE
    // e2eeStatus = -29 → E2EE_DECRYPT_FAILED
    // e2eeStatus = -30 → E2EE_PENDING_DATA
}
```

---

## 8. Điểm chưa xác minh (cần probe live)

1. **Chính xác format ciphertext gửi server**: body là base64(WhisperMessage.serialize()) hay có wrapper thêm?
2. **Group E2EE mechanism**: Sender Key hay per-member X3DH?
3. **wsN-msg.chat.zalo.me**: Protocol có giống `zpw_ws` không? Cmd codes khác?
4. **PreKey server API**: Request/response shape của `/api/e2ee/pc/prekey/update` và `/api/e2ee/pc/prekey/get`?
5. **Key rotation cadence**: Signed prekey rotate mấy ngày? Server enforce hay client-side?

---

*Note: File này là tài liệu nghiên cứu thuật toán từ public GPL source. Không có code nào từ Olaz/libsignal được sao chép vào ZaloLite. Implementation sẽ viết bằng Rust từ standard Signal Protocol spec.*
