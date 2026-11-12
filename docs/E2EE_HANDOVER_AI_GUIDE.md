# Hướng dẫn bàn giao & Các bước tiếp theo sau thử nghiệm E2EE (AI Agent Handover Guide)

> **Dành cho các AI Agent (Gemini, Claude, GPT, v.v.) tiếp quản dự án ctdozlite:**
> Tài liệu này mô tả trạng thái hiện tại của codebase, cách đánh giá kết quả kiểm thử thực tế từ người dùng, và kế hoạch hành động cụ thể cho bước tiếp theo.

---

## 1. Trạng thái hiện tại của Codebase (Baseline Status)

- **Nhánh chính:** `main` (commit `efb68e4` hoặc mới hơn). Toàn bộ các thay đổi P0, P1, P2 và E2EE runtime đã được merge và push lên `origin main`.
- **Kiểm toán biên dịch (Local toolchain):**
  - `cargo fmt --all -- --check`: Đạt 100%.
  - `cargo clippy --all-targets --features experimental-e2ee -- -D warnings`: 0 warning, 0 error.
  - `cargo test --locked --features experimental-e2ee --lib`: 761 passed, 0 failed.
- **Bản build thử nghiệm (Artifact CI):**
  - Workflow: **Windows ARM64 (native build)** trên GitHub Actions (Run ID `43128998884`).
  - Artifact: `ctdozlite-windows-arm64-trial` (gồm `ctdozlite.exe` native ARM64, installer, sha256sums).
- **Cơ chế chạy thử:**
  - Khởi chạy bằng cờ: `.\ctdozlite.exe --gui --e2ee-experimental`
  - Nhật ký ghi tại: `diagnostics/sync-media.jsonl` (truy cập nhanh qua giao diện: *Tiện ích hội thoại* → Tab 7 *Thiết bị & Bảo mật* → *Mở thư mục nhật ký*).

---

## 2. Kịch bản A: Nếu người dùng kiểm thử THÀNH CÔNG (Test Passes)

Người dùng báo tin nhắn gửi từ `ctdozlite` sang Zalo chính thức (PC/Mobile) hiển thị đúng nội dung văn bản, hoặc hai bên gửi/nhận mã hóa trơn tru.

### Các việc AI tiếp theo PHẢI làm ngay:

1. **Ghi nhận bằng chứng thực nghiệm (Empirical Parity E1):**
   - Thu thập dòng log `event: "e2ee"` từ `diagnostics/sync-media.jsonl` của lần chạy thành công (đã khử khóa bí mật/session).
   - Tạo ghi chú bằng chứng mới trong `re-notes/` (hoặc cập nhật `docs/ZALO_REFERENCE_ANALYSIS.md`) xác nhận:
     - Định dạng Signal Protobuf wire (`SIGNAL_PROTO_VERSION = 0x33`).
     - Thuật toán MAC 8-byte (`HMAC-SHA256` truncated).
     - Cấu trúc REST request body `/api/e2ee/pc/message/sms` với `listParams`.
   - Cập nhật trạng thái từ E3 (suy luận) lên E1 (bằng chứng thực nghiệm trực tiếp).

2. **Hoàn thiện luồng nhận tin nhắn thời gian thực qua WebSocket (Inbound Push E2EE):**
   - File liên quan: `src/socket.rs` và `src/gui/worker.rs`.
   - Bắt và xử lý mã lệnh `CMD_SIGNAL_RECEIVE_1_1` (551) khi máy chủ đẩy frame tin nhắn mã hóa về:
     - Lấy ciphertext từ payload WebSocket.
     - Gọi `session.decrypt(...)` từ `src/crypto/session.rs`.
     - Lưu bản rõ đã giải mã vào SQLite `messages` store (`src/store.rs`).
     - Phát `GuiEvent::MessageReceived` để hiển thị tức thì trên giao diện chat.
     - Tự động phản hồi ACK phân phối `CMD_SIGNAL_DELIVER_ACK` (553) và ACK đã xem `CMD_SIGNAL_SEEN_ACK` (554).

3. **Mở rộng sang E2EE Nhóm (Group E2EE / Sender Keys - WP16-3):**
   - Tham khảo mã lệnh `CMD_SIGNAL_RECEIVE_GROUP` (552).
   - Triển khai phân phối Sender Key cho các thành viên trong nhóm theo giao thức Signal Group Messaging.

4. **Nâng cấp Media E2EE (WP16-4):**
   - Mã hóa đính kèm ảnh/file với AES-256-GCM hoặc CBC trước khi tải lên CDN, gắn khóa đối xứng vào payload tin nhắn mã hóa.

5. **Tốt nghiệp tính năng (Graduate from Experimental to Default):**
   - Sau khi test ổn định trên nhiều tài khoản trong 1-2 tuần, gỡ bỏ cờ bắt buộc `--e2ee-experimental` cho 1-1 chat.
   - Cập nhật `docs/releases/v0.008-arm-plan.md` đánh dấu hoàn thành mục E2EE 1-1 Chat.

---

## 3. Kịch bản B: Nếu người dùng gặp lỗi (Test Fails / Bug Encountered)

Người dùng báo lỗi (tin không gửi được, Zalo chính thức báo "Tin nhắn không đọc được", hoặc app báo lỗi đỏ).

### Hướng dẫn chẩn đoán và khắc phục:

1. **Yêu cầu xem log chẩn đoán:**
   - Hướng dẫn người dùng mở thư mục nhật ký qua nút bấm trong app (Tab 7: *Thiết bị & Bảo mật*) và copy các dòng có `"event":"e2ee"` trong `diagnostics/sync-media.jsonl`.
2. **Đối chiếu các giai đoạn (`stage`):**
   - `stage: "upload_bundle"` lỗi: Lỗi khi đăng ký PreKey bundle lên máy chủ. Kiểm tra:
     - Định dạng chữ ký XEdDSA của Identity Key trên Signed PreKey.
     - Số lượng One-Time PreKeys tải lên.
   - `stage: "fetch_bundle"` lỗi: Không lấy được bundle của người nhận. Kiểm tra ID người nhận và query params.
   - `stage: "send"` lỗi (mã HTTP hoặc JSON error): Endpoint `/api/e2ee/pc/message/sms` từ chối payload. Kiểm tra cấu trúc `listParams`, `platform` (1 = PC), `ver`, `iid`.
   - `stage: "decode"` hoặc bên nhận báo "Không giải mã được":
     - Kiểm tra MAC length (8 bytes vs 32 bytes).
     - Kiểm tra Protobuf byte prefix (`0x33`).
     - Kiểm tra thứ tự các trường trong ciphertext protobuf.
3. **Quy tắc sửa lỗi:**
   - Sửa chữa tối giản, tập trung (Surgical change theo Karpathy discipline).
   - Tuyệt đối không tự ý rơi về plaintext khi đang trong hội thoại E2EE (Bảo toàn Invariant chống downgrade).
   - Chạy `cargo clippy --all-targets --features experimental-e2ee -- -D warnings` và `cargo test` trước khi tạo commit mới.
