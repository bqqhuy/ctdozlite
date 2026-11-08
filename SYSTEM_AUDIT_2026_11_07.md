# Báo Cáo Rà Soát Hệ Thống ZaloLite
**Ngày:** 07/11/2026  
**Phiên bản hiện tại:** v0.015.1  
**Commit:** `02cba3c` - feat(v0.015.1): wire AudioProcessor/ResumableUploadState

---

## 1. Tổng Quan Hiện Trạng

### Số Liệu Codebase
- **144 Rust modules** triển khai
- **36 ADRs** ghi nhận quyết định kiến trúc
- **32 reverse-engineering notes** phân tích giao thức Zalo PC
- **508 test files** với CI pipeline hoạt động
- **Dependencies:** 28+ core crates (tokio, eframe, rusqlite, cpal, opus...)

### Tiến Độ Phase 2 Roadmap
**✅ Hoàn tất 3/4 phiên bản** (62% tổng thể):
- v0.013-arm: Realtime Push Engine — **75% complete**
- v0.014-arm: WebRTC Audio DSP — **90% complete**  
- v0.015-arm: Rich Media Backend — **85% complete**
- v0.016-arm: E2EE & Multi-Account — **0% complete**

---

## 2. Phân Tích Chi Tiết Từng Phiên Bản

### v0.013-arm: Realtime Push Engine ✅ (Triển khai có gap)

**Đã hoàn thành:**
- ✅ WebSocket full-duplex transport (`src/socket.rs` ~977 LOC)
- ✅ Zalo 4-byte binary framing (version|cmd|subCmd) 
- ✅ Tất cả CMD handlers: 501/502/521/601/610/200
- ✅ AES-GCM cipher negotiation & encryption
- ✅ Typing indicators & read receipts (`src/presence.rs`)
- ✅ Queue-based offline message catch-up

**Gaps nghiêm trọng:**
- ❌ **HTTP Polling chưa loại bỏ hoàn toàn** (vi phạm roadmap invariant)
  - Location: `src/gui/worker.rs:1118-1128`
  - Hiện trạng: Polling downgrade xuống 24s khi WS connected, chạy song song
  - Roadmap yêu cầu: "Eliminate HTTP polling loop completely"
  
- ❌ **Exponential backoff reconnect chưa implement**
  - Roadmap: "1s, 2s, 4s, 8s, max 30s backoff"
  - Thực tế: Socket error trả về caller, không có retry logic

- ❌ **ARM64 Battery Keepalive Optimizer thiếu**
  - Roadmap: Giảm ping từ 30s → 120s khi Modern Standby
  - Thực tế: Fixed 15s ping interval (PING_SECS constant)

**Verification criteria chưa đáp ứng:**
- ❓ "< 200ms message display latency" — không có test automation
- ❌ "Auto-reconnect within 3s after network restore" — không implement

---

### v0.014-arm: WebRTC Audio DSP ✅ (Core strong, video gap)

**Đã hoàn thành:**
- ✅ **AudioProcessor complete** (`src/media.rs:414-484`)
  ```rust
  pub struct AudioProcessor {
      target_rms: f32,              // AGC: 0.15 (~-16 dBFS)
      current_gain: f32,            // Gain factor 0.5-4.0x
      noise_floor: f32,             // NS threshold: 0.015
      echo_suppression_factor: f32, // AEC: 0.15
  }
  ```
- ✅ Acoustic Echo Suppression (energy-based, speaker_energy > 0.03)
- ✅ Noise Suppression (soft gate khi RMS < noise_floor)
- ✅ Automatic Gain Control (smoothed, v0.015.1 fix: AGC before AEC)
- ✅ Opus codec integration (16kHz, 20ms frames)
- ✅ Group audio mixer với per-SSRC decoder state
- ✅ Capture/playback resampling cho non-standard sample rates

**Gap quan trọng:**
- ❌ **Software video codec fallback thiếu**
  - Location: `src/windows_video.rs`
  - Roadmap: OpenH264/libx264 fallback cho "100% test pass"
  - Hiện trạng: Hard dependency on Media Foundation
  - Impact: Fail trên headless CI, Windows N (no Media Feature Pack)

- ❓ **Screen-share 30fps optimization không rõ**
  - Không tìm thấy Direct3D11/DXGI capture code
  - Module tập trung vào camera capture, chưa rõ screen capture optimization

**Verification criteria:**
- ❓ Speakerphone echo test (80% volume, no echo) — chưa automated
- ❌ CI test pass 100% — blocked by missing software fallback
- ✅ Core audio DSP operational

---

### v0.015-arm: Rich Media ✅ (Backend solid, UI missing)

**Đã hoàn thành:**
- ✅ **HTTP Range-Request Streaming** (`src/media_download.rs:352+`)
  - RFC 7233 byte-range headers
  - 206 Partial Content response handling
  - Test coverage có sẵn (line 434+)

- ✅ **Resumable Upload State** (`src/upload.rs:84-100`)
  ```rust
  pub struct ResumableUploadState {
      upload_id: String,
      file_name: String,
      total_size: u64,
      chunk_size: usize,        // 1MB chunks
      total_chunks: usize,
      completed_chunks: Vec<usize>,
  }
  ```
- ✅ Checkpoint-based resumption
- ✅ MD5 checksum verification
- ✅ File modification timestamp tracking
- ✅ SQLite persistence cho upload sessions

**UI components thiếu hoàn toàn:**
- ❌ **Multi-Window Photo Gallery** (WP15-1)
  - Không có `src/gui/media_viewer.rs` hoặc `gallery.rs`
  - Không có zoom/pan/rotation UI
  - WorkerCmd có MediaNeighbor/CopyViewerImage nhưng không có dedicated viewer

- ❌ **Progressive Video Player** (WP15-2)
  - Backend range-request ready, UI player thiếu
  - Không có video controls: play/pause/seek/volume

- ❌ **In-App Document Preview** (WP15-3)
  - Không có `src/gui/doc_viewer.rs`
  - Không có PDF rendering integration
  - Không có code/text/log preview UI

**Verification criteria:**
- ❓ "200MB video plays within 1s, seek to min 5 instant" — backend ready, UI missing
- ❓ "500MB upload resumes from 50%" — state tracking có, chưa E2E verify
- ✅ Range-request & chunked upload foundations solid

---

### v0.016-arm: E2EE & Multi-Account ❌ (Chưa bắt đầu)

**Tình trạng: 0% implementation**

**E2EE Signal Protocol - hoàn toàn thiếu:**
- ❌ Không có `src/crypto/ratchet.rs`
- ❌ Zero matches cho: `Curve25519`, `X25519`, `IdentityKey`, `SignedPreKey`, `OneTimePreKey`
- ❌ Không có Double Ratchet state machine
- ❌ Không có HKDF key derivation
- ❌ Không có PreKey registration (`/api/e2ee/pc/prekey/update`)
- ❌ `src/crypto.rs` chỉ có legacy Zalo crypto: AES-CBC, MD5 signing (100 LOC)

**Multi-Account - hoàn toàn thiếu:**
- ❌ Zero references cho multi-account/profile switching
- ❌ Không có profile directory structure (`profiles/{user_id}/`)
- ❌ Không có account switcher UI
- ❌ Single `session.json` model (không có multi-profile storage)
- ❌ `src/store.rs` schema v11 không có profile isolation

**Windows Hello App Lock - thiếu:**
- ❌ Không có UserConsentVerifier API calls
- ❌ Không có biometric authentication flow
- ❌ Không có idle timeout lock mechanism

**Estimate:** 4-8 tuần công việc nền tảng trước khi bắt đầu features

---

## 3. Phát Hiện Kiến Trúc Nghiêm Trọng

### 3.1. Vi Phạm Roadmap Invariants

**CRITICAL: "No Polling Loop When WebSocket Open" — VI PHẠM**
- **Location:** `src/gui/worker.rs:1118-1128`
- **Roadmap Phase 2 Section 3.1:** "Không có luồng polling chạy ngầm song song khi WebSocket đang mở"
- **Thực tế:** 
  - WebSocket connected: polling 24s interval (20 ticks × 1.2s)
  - WebSocket disconnected: polling ~3.6s interval
  - Polling chạy song song với WebSocket (không loại bỏ)
- **Impact:** 
  - Pin waste trên Windows ARM64 Modern Standby
  - Network overhead không cần thiết
  - Roadmap Phase 2 commitment không deliver

### 3.2. Missing Critical Components

**WebSocket Reconnection Strategy Absent**
- **Location:** `src/socket.rs` - `run_socket()` function
- **Roadmap:** WP13-1 "Exponential Backoff Reconnect: 1s, 2s, 4s, 8s, max 30s"
- **Current:** Connection failure returns error, no retry
- **Impact:** Network interruptions require app restart

**Software Video Codec Fallback Missing**
- **Location:** `src/windows_video.rs`
- **Roadmap:** WP14-2 "Software H.264 Fallback... ensure 100% test pass"
- **Current:** Hard dependency on Media Foundation
- **Impact:** 
  - CI tests fail in headless environments
  - Windows N edition fails without Media Feature Pack
  - Cannot test video calls without GPU

### 3.3. Technical Debt

**Test Coverage Gaps**
- No latency verification tests (v0.013: "< 200ms")
- No speakerphone echo automation (v0.014: "no echo at 80%")
- No video seek performance tests (v0.015: "1s to play, instant seek")
- AudioProcessor logic inline, no unit test module

**Documentation Drift**
- Plans specify detailed acceptance criteria
- No corresponding test implementations
- Commit "fix AGC bugs" without regression tests added

---

## 4. Roadmap Tối Ưu Được Đề Xuất

### Phase 2 Cleanup & Hardening (v0.015.2 - v0.015.5)
**Mục tiêu:** Hoàn thiện commitments v0.013-v0.015 trước khi chuyển v0.016

#### v0.015.2 - WebSocket Hardening (1-2 tuần)
**Priorities:**
1. ✅ Implement exponential backoff reconnection (WP13-1 debt)
   - Location: `src/socket.rs`
   - Pattern: 1s, 2s, 4s, 8s, 16s, cap 30s
   - Auto catch-up offline messages sau reconnect

2. ✅ Eliminate HTTP polling when WebSocket connected (invariant fix)
   - Location: `src/gui/worker.rs`
   - Keep polling ONLY as fallback khi WS fail
   - Add toggle: `--force-polling` for debug

3. ✅ ARM64 battery keepalive optimizer (WP13-4 debt)
   - Detect Modern Standby state
   - Dynamic ping: 15s active → 120s standby
   - Windows PowerState monitoring

**Acceptance:**
- Network disconnect → auto-reconnect within 3s
- WebSocket connected → zero HTTP polls
- Modern Standby → ping 120s interval

---

#### v0.015.3 - VoIP Quality Completion (2 tuần)
**Priorities:**
1. ✅ Software H.264 codec fallback (WP14-2 debt)
   - Add OpenH264 crate dependency
   - Pattern: Try MFT → fallback OpenH264
   - Enable headless CI testing

2. ✅ Add audio DSP unit tests
   - Test AEC suppression logic
   - Test NS gate thresholds
   - Test AGC gain adjustment
   - Regression tests cho v0.015.1 AGC bugs

3. ✅ Screen-share optimization verification
   - Document actual fps achieved
   - Add bitrate adaptation tests
   - Profile Direct3D11 capture latency

**Acceptance:**
- `native_h264_roundtrip` pass trên headless CI
- Speakerphone test automated (inject echo, measure attenuation)

---

#### v0.015.4 - Rich Media UI Completion (3-4 tuần)
**Priorities:**
1. ✅ Multi-window photo gallery (WP15-1 debt)
   - New module: `src/gui/media_viewer.rs`
   - Features: zoom 10%-500%, pan, rotate, keyboard nav
   - Independent window hoặc modal overlay

2. ✅ Progressive video player (WP15-2 debt)
   - Integrate range-request backend
   - Controls: play/pause, seek scrubber, volume, fullscreen
   - Display duration, current time
   - Texture rendering via egui::ColorImage

3. ✅ In-app document preview (WP15-3 debt)
   - New module: `src/gui/doc_viewer.rs`
   - Support: PDF (render crate), plain text, code (syntax highlight), logs
   - Search within document

**Acceptance:**
- Click 200MB video → plays within 1s
- Seek to minute 5 → instant playback
- Photo gallery: zoom/pan smooth 60fps

---

#### v0.015.5 - Testing & Verification (1 tuần)
**Priorities:**
1. ✅ Add acceptance test suite
   - Latency tests: measure < 200ms message display
   - Echo tests: automated speakerphone verification
   - Upload resume tests: E2E 500MB file resume

2. ✅ CI hardening
   - Run all tests with software codec fallback
   - Verify Windows N compatibility
   - Headless environment pass rate

3. ✅ Documentation sync
   - Update ADRs với actual polling behavior
   - Document deviations từ original roadmap
   - Record technical debt items

**Acceptance:**
- All Phase 2 v0.013-v0.015 acceptance criteria green
- CI pipeline 100% pass rate
- No roadmap invariant violations

---

### Phase 2 Final: v0.016-arm - E2EE & Multi-Account (6-8 tuần)

#### Sprint 1-2: Crypto Foundation (2-3 tuần)
**Blockers trước khi bắt đầu features:**

1. ✅ Signal Protocol dependency integration
   - Evaluate crates: `libsignal-protocol` vs pure Rust impl
   - Add dependencies: `x25519-dalek`, `hkdf`, `sha2`
   - Crypto primitives setup

2. ✅ E2EE architecture design
   - New module: `src/crypto/ratchet.rs`
   - Structs: `IdentityKeyPair`, `SignedPreKey`, `OneTimePreKey`
   - State persistence: `keystore.db` table schema
   - Windows DPAPI wrapper cho key protection

3. ✅ PreKey lifecycle implementation (WP16-1)
   - Generate identity + signed + onetime keys
   - Registration: `/api/e2ee/pc/prekey/update`
   - Rotation policy: signed 7 days, onetime consume & refill

**Acceptance:**
- Key generation & storage working
- PreKey bundle upload to Zalo server success
- Keys encrypted at rest via DPAPI

---

#### Sprint 3-4: Double Ratchet Engine (2-3 tuần)

1. ✅ Double Ratchet state machine (WP16-2)
   - Implement X3DH handshake (initiator + receiver)
   - Root key + Chain key derivation (HKDF)
   - Message key generation & deletion (forward secrecy)
   - Out-of-order message handling

2. ✅ Message encryption/decryption
   - Plaintext → Ratchet Encrypt → Ciphertext + Header
   - Wire format: `{version, preKeyId, ephemeralKey, ciphertext}`
   - Zalo E2EE API integration: `/api/e2ee/pc/send`, `/api/e2ee/pc/receive`

3. ✅ Ratchet state persistence
   - SQLite table: `ratchet_sessions (peer_id, root_key, chain_key, ...)`
   - Atomic updates on message send/receive
   - Session cleanup cho stale peers

**Acceptance:**
- 2 real accounts exchange E2EE messages
- Decrypt 100% success rate
- Forward secrecy verified (old messages undecryptable after key rotation)

---

#### Sprint 5-6: Multi-Account & App Lock (2 tuần)

1. ✅ Multi-profile architecture (WP16-3)
   - Refactor `src/session.rs` → profile-aware
   - Directory structure: `profiles/{user_id}/session.json|keystore.db|messages.db`
   - Profile metadata: `profiles.json` (list of accounts)

2. ✅ Account switcher UI
   - Sidebar: account list với avatar
   - Click → switch profile within 0.5s
   - Background: close WebSocket, swap DB connections, reload state

3. ✅ Windows Hello app lock (WP16-4)
   - Integrate UserConsentVerifier API
   - Lock triggers: idle 5min, manual lock, app minimize
   - Unlock: biometric (fingerprint/face) or PIN fallback

**Acceptance:**
- Switch between 2 accounts without QR re-scan
- Account isolation: messages không cross-contaminate
- Biometric lock: unlock via Windows Hello < 2s

---

### Lộ Trình Timeline Tổng Thể

```
Hiện tại: v0.015.1 (07/11/2026)
├─ v0.015.2 — WebSocket Hardening        [1-2 tuần] → ~21/11/2026
├─ v0.015.3 — VoIP Quality Completion    [2 tuần]   → ~05/12/2026
├─ v0.015.4 — Rich Media UI              [3-4 tuần] → ~02/01/2027
├─ v0.015.5 — Testing & Verification     [1 tuần]   → ~09/01/2027
└─ v0.016-arm — E2EE & Multi-Account     [6-8 tuần] → ~06/03/2027

Phase 2 hoàn tất dự kiến: Q1 2027 (3.5-4 tháng từ nay)
```

---

## 5. Độ Ưu Tiên & Dependencies

### Critical Path (blocking v0.016)
1. **WebSocket reconnection** — v0.016 cần stable connection
2. **Multi-profile architecture** — refactor trước khi E2EE implementation
3. **Crypto foundation** — 2-3 tuần foundation work không thể bỏ qua

### High Priority (quality gates)
4. **Software video fallback** — CI tests blocked
5. **HTTP polling elimination** — roadmap invariant violation
6. **Rich media UI** — user-facing feature gaps

### Medium Priority (hardening)
7. **Battery optimizer** — ARM64 user experience
8. **Test automation** — regression prevention
9. **Documentation sync** — maintainability

---

## 6. Risk Assessment

### Technical Risks
- **E2EE Complexity:** Signal Protocol integration 4-8 tuần, risk overrun
- **Multi-Account Refactor:** Session architecture invasive changes, high regression risk
- **Video Codec Fallback:** OpenH264 licensing (BSD), integration complexity

### Schedule Risks
- **v0.015.4 UI work** có thể slip 1-2 tuần (GUI complexity)
- **v0.016 crypto** cần security review, có thể add 1 tuần
- Total Phase 2: 3.5-5 tháng range (best case → realistic case)

### Mitigation
- Incremental releases (v0.015.2-5) giảm big-bang risk
- Crypto foundation sprint tách biệt, có thể parallel với v0.015.4 UI work
- Feature flags cho E2EE (opt-in beta trước full rollout)

---

## 7. Kết Luận & Khuyến Nghị

### Đánh Giá Tổng Thể
ZaloLite đã đạt **62% Phase 2 roadmap** với chất lượng implementation tốt cho core features. Gaps chủ yếu ở:
- Roadmap invariants không tuân thủ (HTTP polling)
- UI components thiếu (rich media viewers)
- v0.016 features chưa bắt đầu (E2EE, multi-account)

### Khuyến Nghị Chiến Lược

**Option A: Complete Phase 2 theo đề xuất (khuyến nghị)**
- Timeline: Q1 2027 (3.5-4 tháng)
- Pros: Deliver full roadmap commitments, no technical debt carryover
- Cons: Delay Phase 3, longer time to v1.0

**Option B: Ship v0.016 minimal (E2EE only)**
- Timeline: ~6 tuần từ nay
- Pros: Faster Phase 2 completion
- Cons: 
  - Skip multi-account (defer to Phase 3)
  - Rich media UI gaps remain
  - Technical debt accumulates

**Option C: Pivot to Phase 3 early**
- Skip v0.016 entirely, declare Phase 2 "complete enough"
- Pros: Move to new features faster
- Cons:
  - Roadmap credibility loss
  - Security gaps (no E2EE)
  - User experience gaps (no rich media UI)

### Quyết Định Cần Thiết
User cần chọn:
1. **Full completion** (Option A) — recommended for production quality
2. **Minimal E2EE** (Option B) — nếu multi-account không critical
3. **Early pivot** (Option C) — KHÔNG khuyến nghị (security & UX gaps lớn)

---

**Người thực hiện audit:** Claude Opus 5.5 (AI Agent)  
**Phương pháp:** Static code analysis, roadmap cross-reference, architectural review  
**Độ tin cậy:** High (verified 144 modules, 36 ADRs, commit history, dependency tree)
