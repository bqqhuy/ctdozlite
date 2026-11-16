# BIÊN BẢN ĐỒNG THUẬN KIỂM TOÁN ĐỘC LẬP (3 PRO AUDITORS CONSENSUS)
**Ngày kiểm toán:** 17/11/2026  
**Nhánh kiểm tra:** `feature/audit-remediation-2026-11-17`  
**Commit cơ sở:** `4496952` (`fix(remediation): memory permit bounding, disk cache, blocking offload, and privacy temp lifecycle (Phases 2 & 3)`)  
**Mục tiêu:** Khắc phục triệt để các tồn đọng phát hiện trong đợt kiểm chứng độc lập snapshot audit `e75e372` ngày 17/11/2026.

---

## 1. HỘI ĐỒNG KIỂM TOÁN ĐỘC LẬP

Hội đồng kiểm toán gồm 3 Senior Gemini Pro Auditors hoạt động độc lập, tự chạy các lệnh kiểm thử và thanh tra chi tiết mã nguồn theo các chuyên môn riêng biệt:

1. **Auditor 1:** Senior Protocol & Security Auditor (`18127bf6-1a4f-4249-9b94-39d6fa99b753`)
   - Chuyên trách: A1 (Signal Protobuf recursion depth limit), A2 / M-05 (Clipboard & screen capture temp privacy & account isolation), A3 / M-01 (E2EE Reset requirement state sync).
   - Quyết định: **[APPROVE]**
2. **Auditor 2:** Senior Graphics & Performance Auditor (`004844e5-58ba-49ec-b1c2-14fa3d1c9497`)
   - Chuyên trách: B01 (Ami sticker GIF downscale & GPU texture limit 8192px), B02 (MediaJobPermit lifecycle & semaphore backpressure), B03 (Offline disk cache), B04 (CPU image decode offload sang `spawn_blocking`).
   - Quyết định: **[APPROVE]**
3. **Auditor 3:** Senior UI/UX & Runtime Auditor (`842dfc0d-8605-4523-83cb-9f58e6c8ba7d`)
   - Chuyên trách: B06 (Adaptive height cho sync status bar trên viewport nhỏ/màn hình DPI cao), B05 (Offload TUI logout sang background thread), Phản biện bài test hồi quy cô lập tài khoản `test_m05_clipboard_temp_file_account_isolation`.
   - Quyết định: **[APPROVE]**

---

## 2. KẾT QUẢ KIỂM TOÁN TỪNG HẠNG MỤC

### Giai đoạn 1: Fix Blockers & Critical Bounds (P1)
- **B06 — Adaptive height cho thanh trạng thái đồng bộ:**
  - *Giải pháp:* Tại `src/gui/workspace.rs`, áp dụng `compact_h = ctx.screen_rect().height() < 300.0`. Khi viewport nhỏ, chiều cao co từ 92px xuống 34px (dạng 1 dòng `ui.horizontal` gọn gàng).
  - *Kiểm chứng:* Chạy smoke test 480x360 scale 1.5 với cờ `--ui-smoke-test` và `--ui-smoke-phone-sync` cho kết quả **Exit 0 thành công** (trước đây thất bại Exit 3 do nút Gửi bị tràn ra ngoài clip rect).
- **B01 — Giới hạn kích thước Sprite Sheet cho Sticker động Ami:**
  - *Giải pháp:* Tại `src/bundled_stickers.rs`, downscale frame GIF về `target_w = 160px` khi frame gốc > 160px hoặc tổng bề ngang > 4096px. Bổ sung guard 8192px tại `bundled_stickers.rs`, `store_texture` và `decode_sprite_from_bytes`.
  - *Kiểm chứng:* Test `test_decode_gif_sprite` kiểm tra toàn bộ 15 asset Ami, tất cả đều $\le 8192\text{px}$. Ami #02 giảm từ $12.800\text{px}$ (28,67 MB) xuống $3.200\text{px}$ (1,79 MB), tiết kiệm 93,7% VRAM/RAM.
- **A1 — Giới hạn độ sâu đệ quy giải mã Signal Protobuf Wire:**
  - *Giải pháp:* Tại `src/crypto/wire.rs`, hàm `deserialize_signal_proto_with_depth` giới hạn `depth <= 2`, cấm tuyệt đối `PreKeySignalMessage` lồng nhau ở `depth > 0`.
  - *Kiểm chứng:* Test `test_prekey_recursion_depth_capped` **PASSED**.

### Giai đoạn 2: Memory Backpressure, Caching & Concurrency (P2)
- **B02 — Giữ `MediaJobPermit` trong `StickerSpriteReady`:**
  - *Giải pháp:* `StickerSpriteReady` giữ `permit: Option<MediaJobPermit>` cho đến khi UI nạp xong vào GPU qua `store_texture` mới drop. Đảm bảo giới hạn số lượng tác vụ giải mã đồng thời không vượt quá năng lực renderer. Zero permit leak và zero deadlock.
- **B03 — Khôi phục Offline Disk Cache cho Sticker Sprites:**
  - *Giải pháp:* Tại `src/gui/worker.rs`, kiểm tra cache đĩa `{key}.png` trước khi gọi network. Lưu cache đĩa nguyên tử thông qua `storage::atomic_write`.
- **B04 — Offload CPU-intensive decode sang `spawn_blocking`:**
  - *Giải pháp:* Toàn bộ các thao tác giải mã GIF, PNG thumbnail và sprite bytes được bọc bằng `tokio::task::spawn_blocking`, giải phóng Tokio 2-thread runtime để xử lý socket real-time.
- **B05 — TUI Logout chạy trên background thread:**
  - *Giải pháp:* Tại `src/tui.rs`, gọi `profiles::perform_account_logout` trong `std::thread::spawn`. Luồng chính TUI lập tức dọn sạch trạng thái bộ nhớ và cập nhật thông báo giao diện.

### Giai đoạn 3: Quyền riêng tư Tệp tạm & Nhất quán Trạng thái E2EE (P2)
- **A2 / M-05 — Cách ly và dọn dẹp tệp tạm Clipboard / Screen capture:**
  - *Giải pháp:* Truyền `owner` vào `read_clipboard_image_to_png` và `finalize_clipboard_rgba_image`, lưu ảnh tạm vào `storage::account_temp_dir(owner)`. Dọn dẹp tệp tạm khi thay thế ảnh, xóa draft, đăng xuất hoặc đổi tài khoản.
  - *Kiểm chứng:* Test `test_m05_clipboard_temp_file_account_isolation` **PASSED**.
- **A3 / M-01 — Reset yêu cầu E2EE đồng bộ hai chiều:**
  - *Giải pháp:* Khi bấm "Gỡ yêu cầu E2EE", UI gửi `WorkerCmd::ResetE2eeRequirement`, hiển thị toast đang xử lý. Worker gọi cập nhật DB trên `spawn_blocking` và phản hồi qua `GuiEvent::E2eeRequirementResetResult`. UI chỉ xóa `trial_peers` khi DB xác nhận thành công.

---

## 3. TỔNG HỢP KIỂM TRA TỰ ĐỘNG CỤC BỘ

| Công cụ / Lệnh kiểm tra | Kết quả | Ghi chú |
|---|---|---|
| `cargo fmt --all -- --check` | **Exit 0** | Codebase tuân thủ format chuẩn |
| `cargo check --all-targets --all-features --locked` | **Exit 0** | Không có lỗi compile |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | **Exit 0** | 0 warning, 0 error |
| `cargo test --locked --no-fail-fast` | **Exit 0** | Tất cả các suite unit & regression test đều PASS (851 lib tests, 23 audit regressions, 19 updater, 15 message identity, 9 media resources, 3 a18 tests...) |
| `python -I scripts/tests/test_audit_prepush.py` | **Exit 0** | 4 tests OK |
| Smoke test 480x360 scale 1.5 | **Exit 0** | B06 khắc phục hoàn toàn (trước đây exit 3) |

---

## 4. KẾT LUẬN & ĐỒNG THUẬN CHUNG

Hội đồng 3 Gemini Pro Auditors độc lập bỏ phiếu:
- Senior Protocol & Security Auditor: **[APPROVE]**
- Senior Graphics & Performance Auditor: **[APPROVE]**
- Senior UI/UX & Runtime Auditor: **[APPROVE]**

**Kết quả: ĐẠT ĐỒNG THUẬN TUYỆT ĐỐI (100% UNANIMOUS APPROVAL).**  
Đủ điều kiện thực hiện merge vào nhánh `main` và push lên GitHub remote repository.
