# AI Wiring Discipline — Nguyên tắc chống Backend Stub và Test-Only Code

> **Hiệu lực**: Bắt buộc cho mọi AI agent tham gia phát triển ZaloLite.
>
> **Ngày ban hành**: 07/11/2026, sau audit phát hiện 3 critical findings trong v0.013–v0.015.
>
> **Mục đích**: Ngăn chặn hiện tượng "code tồn tại + test pass nhưng không chạy thật trong app"
> (backend stub / test-only code) — vi phạm tinh thần bất biến "No UI Stubs" trong [AGENTS.md](../AGENTS.md).

---

## Vấn đề gốc rễ: "Test-Driven False Positive"

### Trường hợp điển hình (phát hiện từ audit 07/11/2026):

**Commit tuyên bố**: "feat(voip): implement WebRTC DSP AudioProcessor (AEC/NS/AGC)"

**Thực tế trong code**:
```rust
// src/media.rs:413
pub struct AudioProcessor { /* ... */ }
impl AudioProcessor {
    pub fn process_capture(&mut self, samples: &mut [f32], speaker_energy: f32) {
        // AEC/NS/AGC logic — 94 dòng, hoàn chỉnh
    }
}

#[test]
fn audio_processor_cancels_echo_and_suppresses_noise() {
    let mut ap = AudioProcessor::new();
    ap.process_capture(&mut samples, speaker_energy);
    assert!(echo suppressed); // ✅ PASS
}
```

**Nhưng đường chạy thật** (src/media.rs:1264):
```rust
macro_rules! capture_stream {
    ($sample:ty,$device:ident,$config:ident) => {{
        $device.build_input_stream(&$config, move |data: &[$sample], _| {
            // Microphone → downmix → resample → buffer
            // ❌ KHÔNG HỀ GỌI AudioProcessor::process_capture
        }, |e| eprintln!(...), None)
    }}
}
```

**Kết quả**: Test pass. Clippy clean. Commit message hợp lệ. CHANGELOG tuyên bố "Triệt tiêu
tiếng vọng khi bật loa ngoài" — **nhưng runtime không làm gì cả**. Người dùng vẫn nghe vọng.

---

## Nguyên tắc bắt buộc cho mọi AI agent

### 1. Định nghĩa "wired" vs "test-only"

| Trạng thái | Dấu hiệu | Được phép commit? |
| :--- | :--- | :--- |
| **Wired** | Code được gọi từ đường chạy thật (UI event → Worker → API/media/crypto → output) | ✅ Được commit + tuyên bố tính năng |
| **Test-only** | Chỉ test của chính nó gọi; grep usage trả về `mod tests` duy nhất | ❌ Cấm tuyên bố tính năng — gắn nhãn `#[allow(dead_code)]` + TODO wiring |
| **Stub backend** | Có function public nhưng không consumer nào gọi nó ngoài test | ❌ Giống test-only — vi phạm "No UI Stubs" spirit |

### 2. Checklist bắt buộc trước mỗi commit tính năng

Trả lời **CẢ 5 CÂU HỎI** này trước khi viết commit message:

#### Q1 — "Tôi vừa viết function `foo()`. Ai gọi nó?"
```bash
rg "foo\(" --type rust | grep -v "mod tests" | grep -v "^src/module.rs.*pub fn foo"
```
Nếu **chỉ có dòng định nghĩa** hoặc **chỉ có test** → test-only, chưa được tuyên bố.

#### Q2 — "Đường từ UI → function này có đi qua không?"
Ví dụ `AudioProcessor::process_capture`:
- UI: người dùng bật mic trong cuộc gọi
- Worker: `WorkerCmd::StartCall`
- Runtime: `media.rs::start_call` → `capture_stream!` macro
- **Trong macro đó, có gọi `.process_capture()` không?** ← phải grep chính xác macro body.

Nếu **không tìm thấy call-site trong đường UI→runtime** → stub.

#### Q3 — "Test của tôi có dùng mock/fixture hay đường thật?"
```rust
// ❌ BAD: test gọi function trực tiếp (unit test — không chứng minh wiring)
#[test]
fn test_foo() {
    let result = foo(42);
    assert_eq!(result, 84);
}

// ✅ GOOD: test đi qua integration path (chứng minh wiring)
#[test]
fn test_call_uses_audio_processor() {
    let call = setup_test_call();
    call.send_audio_frame(samples);
    // Assert rằng AudioProcessor đã được gọi (qua side-effect observable:
    // echo suppressed, hoặc log, hoặc counter tăng)
}
```

Nếu test chỉ gọi function trực tiếp → **chưa chứng minh wiring** → cần thêm integration test.

#### Q4 — "Tính năng cần state/storage không? Đã wire vào SQLite chưa?"
Ví dụ `ResumableUploadState`:
- Kế hoạch (WP15-4) yêu cầu: bảng `upload_sessions` lưu checkpoint.
- Thực tế: `impl ResumableUploadState` hoàn chỉnh, test pass — **nhưng không có
  CREATE TABLE, không có INSERT/SELECT nào trong store.rs**.
- Kết quả: rớt mạng = mất state, resume không hoạt động.

**Rule**: Nếu struct có `#[derive(Serialize, Deserialize)]` và tên chứa "State"/"Session"/"Cache"
→ bắt buộc có persistence code (`store.rs` hoặc file) hoặc ghi rõ "in-memory only, ephemeral".

#### Q5 — "CHANGELOG tuyên bố X. Runtime thật có làm X không?"
Ví dụ từ v0.015:
- CHANGELOG: "Tua video tức thì qua Range request"
- Code: `range_request()` function tồn tại, marked `#[allow(dead_code)]`
- **Không có video player nào gọi nó** → tuyên bố sai.

**Rule**: CHANGELOG chỉ được viết **sau khi grep confirm có consumer thật**.

---

## 3. Khi nào được phép commit code chưa wire?

**Được phép** trong các trường hợp sau (phải gắn nhãn rõ ràng):

### Trường hợp A — Refactor prep (internal, private)
```rust
// Tách logic phức tạp thành helper internal — chưa public, chưa tuyên bố tính năng
fn extract_helper_for_future_refactor() { /* ... */ }
```
Commit message: `refactor: extract helper (internal prep, not wired yet)`

### Trường hợp B — Struct + test trước, wiring sau (gắn TODO)
```rust
/// Resumable upload state tracker.
/// TODO(wiring): integrate into upload flow + SQLite persistence (tracked in issue #NNN)
#[allow(dead_code)] // Remove when wired
pub struct ResumableUploadState { /* ... */ }
```
Commit message: `feat(upload): add ResumableUploadState struct (wiring in next commit)`
CHANGELOG: **KHÔNG ghi** "Resumable upload hoạt động" — chỉ ghi khi wiring commit landed.

### Trường hợp C — Feature flag gated (opt-in testing)
```rust
if std::env::var("EXPERIMENTAL_AEC").is_ok() {
    audio_processor.process_capture(&mut samples, speaker_energy);
}
```
Commit message: `feat(voip): add experimental AEC behind EXPERIMENTAL_AEC flag`
CHANGELOG: Ghi rõ "**Experimental**, không mặc định, cần flag `EXPERIMENTAL_AEC=1`."

**Cấm tuyệt đối**: Commit code hoàn chỉnh (public API + test pass) mà không có wiring,
không có `#[allow(dead_code)]`, không có TODO, rồi tuyên bố tính năng trong CHANGELOG.

---

## 4. Kỹ thuật "Wiring-First" thay vì "Test-First"

### Anti-pattern (gây ra audit findings):
1. Viết struct/function hoàn chỉnh
2. Viết unit test → pass
3. Commit với message "implement X"
4. **Quên wire** hoặc "để sau"
5. CHANGELOG tự động gen từ commit message → tuyên bố sai

### Pattern đúng (Wiring-First):
1. **Tìm call-site trước** — grep đường UI→runtime, xác định chỗ cần chèn logic mới
2. Viết stub tại call-site:
   ```rust
   // capture_stream! macro
   for sample in samples {
       buffer.push(sample); // ← OLD
       // TODO: call AudioProcessor here
   }
   ```
3. Implement logic (struct/function)
4. **Wire ngay** — chèn vào call-site đã identify ở bước 1
5. Test integration (đi qua đường UI→runtime)
6. Commit: code + wiring cùng lúc

**Benefit**: Không bao giờ có commit "tính năng hoàn chỉnh" mà thiếu wiring.

---

## 5. Review checklist cho AI reviewing AI khác

Khi review commit/PR của AI khác, kiểm tra:

- [ ] Commit message claim "implement X" → grep `rg "X\(" --type rust` có consumer ngoài test không?
- [ ] Struct có `State`/`Session` trong tên + `Serialize` → có code persistence không?
- [ ] Function public mới → có integration test đi qua đường UI→runtime không?
- [ ] CHANGELOG tuyên bố tính năng → code path thật có reach được function đó không?
- [ ] Nếu function marked `#[allow(dead_code)]` → commit message có ghi "not wired yet" không?

**Nếu 1 trong 5 mục fail → block merge**, yêu cầu sửa commit message + CHANGELOG hoặc wire code.

---

## 6. Template commit message trung thực

### ✅ Commit hoàn chỉnh (wired):
```
feat(voip): wire AudioProcessor into capture path for echo cancellation

- Integrate AudioProcessor::process_capture into capture_stream! macro
- Add speaker energy reference from playback buffer
- Integration test: test_call_with_speaker_suppresses_mic_echo passes
- CHANGELOG: "Triệt tiêu tiếng vọng (energy-gate AEC, fork deviation)"
```

### ✅ Commit partial (struct only, wiring later):
```
feat(upload): add ResumableUploadState struct (not wired yet)

- Implements chunk tracking logic + next_missing_chunk
- Unit tests pass
- TODO: wire into upload flow + SQLite persistence (issue #123)
- CHANGELOG: NOT updated (will update when wired)
```

### ❌ Commit sai (gây audit finding):
```
feat(voip): implement WebRTC DSP AudioProcessor (AEC/NS/AGC)

- AudioProcessor struct with process_capture
- Tests pass
(Missing: không wire vào capture path → runtime không gọi → CHANGELOG claim false)
```

---

## 7. Khắc phục nhanh khi phát hiện unwired code

Nếu audit/review phát hiện function chưa wire:

**Option A — Wire ngay** (nếu scope nhỏ, <50 dòng):
```bash
# Tìm call-site
rg "capture_stream|build_input_stream" src/
# Wire vào đúng chỗ
# Commit: "fix(voip): wire AudioProcessor (missing from feat commit abc123)"
```

**Option B — Revert tuyên bố** (nếu wiring phức tạp, cần nhiều thời gian):
```bash
# Đánh dấu dead_code
git diff  # thêm #[allow(dead_code)] vào struct
# Sửa CHANGELOG: gỡ tuyên bố tính năng, ghi "Prepared but not wired (v0.X+1)"
# Commit: "fix(changelog): mark AudioProcessor as unwired prep (issue #123)"
```

**Option C — Gỡ hẳn** (nếu không plan wire trong 2 bản tới):
```bash
git revert <commit-hash>
# Commit message: "Revert 'feat(X)': code was test-only, not wired to runtime"
```

**Cấm**: Giữ nguyên code unwired + CHANGELOG sai → tích lũy nợ kỹ thuật.

---

## 8. Ví dụ thực tế từ audit 07/11/2026

| Function | Claim | Reality | Đáng lẽ phải |
| :--- | :--- | :--- | :--- |
| `AudioProcessor::process_capture` | "Triệt tiêu vọng âm khi bật loa" (v0.014) | Test-only, capture path không gọi | Wire vào `capture_stream!` hoặc mark `#[allow(dead_code)] // TODO: wire in v0.015.1` |
| `range_request` | "Tua video tức thì" (v0.015) | `#[allow(dead_code)]`, no player | Hoặc wire vào player, hoặc gỡ khỏi CHANGELOG |
| `ResumableUploadState` | "Tiếp tục upload từ vị trí ngắt" (v0.015) | Không persist SQLite, rớt mạng = mất state | Thêm `upload_sessions` table + INSERT/SELECT |

**Bài học**: Test pass + clippy clean **không đủ** — phải grep consumer thật.

---

## Tóm tắt ngắn gọn (dán vào đầu mỗi session làm việc)

> 1. **Grep consumer trước khi claim**: `rg "function_name\(" --type rust | grep -v test`
> 2. **Trace UI→runtime path**: function có trong đường thật không?
> 3. **Integration test > unit test**: test phải đi qua call-site thật
> 4. **State needs storage**: struct `*State` phải có persistence hoặc ghi "ephemeral"
> 5. **CHANGELOG = runtime truth**: chỉ viết khi code chạy thật, không viết từ commit message
> 6. **Wiring-First**: tìm call-site → stub → implement → wire → test integration → commit
> 7. **Unwired code phải marked**: `#[allow(dead_code)] // TODO wiring issue #N`

**Câu hỏi tự kiểm tra nhanh**: "Nếu tôi mở app, click nút X, function này có chạy không?"
Nếu **"không biết"** hoặc **"có lẽ không"** → chưa được commit với tuyên bố tính năng.
