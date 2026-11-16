# Kết quả hai Pro auditor và vòng sửa

Phạm vi review: `f0d3206..99674a0` trên `codex/audit-21-remediation`. Hai auditor chạy độc lập, chỉ đọc (không sửa file). Auditor 2 chạy thêm test tập trung: 91 passed, 0 failed.

## Vòng 1 — kết luận

| | Auditor 1 | Auditor 2 |
|---|---|---|
| Kết luận | CHANGES REQUIRED | APPROVE |
| Major | 3 | 0 |
| Minor | 9 | 5 |

Hai bên thống nhất: F07, F11–F17, F19 đóng ở mức code; F03–F06, F08, F09 còn một phần vì thiếu test boundary/race/đo lường. Khác biệt: F10 (A1 tìm thêm nhãn sai trong drawer), F18 (A1 tìm hai đường parse TUI), F02/F20/F21 (mỗi bên chặt ở một mục). Tracker lấy kết luận chặt hơn.

## Lỗi được nêu và cách xử lý trong R1

| # | Nguồn | Mức | Lỗi | Xử lý R1 |
|---|---|---|---|---|
| 1 | A1 | major | Phím `y` của TUI bị chuyển sang “gửi lại”, mất chức năng trả lời | Gửi lại chuyển sang `Y`; `y` trả lời như baseline; help cập nhật |
| 2 | A1, A2 | major | `Hydrated` thay toàn bộ thread, làm mất tin lạc quan và outcome F05 | `merge_hydrated` hợp nhất bằng `merge_thread_into`; test mới |
| 3 | A1 | major | Drawer vẫn ghi “🔒 Mã hóa đầu cuối: Tiêu chuẩn” cho mọi hội thoại (F10) | `e2ee_security_summary` theo trạng thái thật; test ma trận 16 trường hợp |
| 4 | A1 | minor | Logout khi lệnh xóa đã ACK còn chạy làm mất tombstone | Chặn logout khi còn tác vụ gửi/xóa quan trọng |
| 5 | A1 | minor | Snapshot cũ có thể ghi sau snapshot mới | Snapshot mới được nhận thì bỏ snapshot cũ đang chờ |
| 6 | A1 | minor | TUI refresh / group history không dùng account UID (F18) | Dùng biến thể `_with_profile` |
| 7 | A1 | minor | Double-click reaction gửi “add” hai lần | Chặn dispatch khi intent của tin đó còn pending (worker luôn trả kết quả) |
| 8 | A1 | minor | Start voice bị bỏ qua không trả event, UI kẹt | Luôn trả `VoiceEvent::Failed` |
| 9 | A1, A2 | minor | Gửi sticker bỏ qua gate hội thoại ẩn | Thêm `selected_thread_allowed()` |
| 10 | A1, A2 | minor | Batch tin lúc khởi động không được lưu | `persist_async()` sau `Hydrated` |
| 11 | A1 | minor | Lỗi Windows Hello bị gộp thành “Xác thực thất bại” | Mang thông điệp lỗi vào `AppUnlockFailed` |
| 12 | A1, A2 | minor | Tracker lỗi thời | Cập nhật IMPLEMENTATION_TRACKER.md |
| 13 | A2 | minor | Lỗi ghi first ping / GET_MSG bị `let _ =` | Propagate bằng `?` |
| 14 | A2 | minor | Màn khóa không có đường đăng xuất khi WinRT treo; xác thực >30 s bị coi là lỗi | **Chưa sửa** — rủi ro còn lại, cần native ARM64 để thiết kế; xem bàn giao |

## Re-review R1 (`99674a0..a41b49c`)

| | Auditor 1 | Auditor 2 |
|---|---|---|
| Kết luận | **APPROVE** | **APPROVE** |
| Kiểm chứng | Đọc code; 12/12 mục fixed | Đọc code; test tập trung 51 passed, 0 failed |
| Lỗi mới | 2 minor | 2 minor |

Lỗi mới và xử lý:

| Nguồn | Lỗi | Xử lý |
|---|---|---|
| A1, A2 | Tracker link tới `ANTIGRAVITY_HANDOVER.md` chưa được commit | File được commit cùng vòng R2 |
| A1 | `merge_hydrated` không áp bộ lọc `deleted_messages` trong bộ nhớ (chỉ an toàn nhờ thứ tự hàng đợi) | R2: áp `filter_deleted_replays` sau merge |
| A2 | Khóa reaction pending chỉ được nhả nhờ worker trả lời hoặc logout | Rủi ro còn lại (không có đường lỗi hiện tại); ghi vào bàn giao |

Hai auditor thống nhất: không còn blocker/major. F21 (đường đăng xuất trên màn khóa khi WinRT treo) giữ là rủi ro còn lại, phải nằm trong kế hoạch nghiệm thu native ARM64.

---

# Kết quả Review Độc lập — Audit `bd44cc1` Phase 1 (Security Fixes)

Phạm vi review: Commit `e6803ee` (C-01) và `bd5f9c8` (H-01) trên nhánh `antigravity/phase-1-security-fixes`.
Hai Pro auditor (`Phase 1 Pro Auditor 1` và `Phase 1 Pro Auditor 2`) đã tiến hành kiểm toán độc lập, chạy test và rà soát diff.

## Đánh giá tổng hợp Phase 1

| Chỉ số | Pro Auditor 1 | Pro Auditor 2 |
|---|---|---|
| **Kết luận** | **APPROVE** | **APPROVE** |
| **Blocker** | 0 | 0 |
| **Major** | 0 | 0 |
| **Minor** | 2 | 2 |

### Xác nhận C-01:
- Cắt slice an toàn `data.get(start..end)`, `checked_add` triệt tiêu mọi phép toán tràn số u64/usize.
- `decode_varint_u32` từ chối giá trị > `u32::MAX`.
- Early gate tại `src/chat.rs`: tin nhắn văn bản thuần không có metadata trong chat thông thường không bao giờ đi vào parser E2EE wire.
- Fixture test PoC pass cả debug và release. Fuzz test 10,000 cases ngẫu nhiên pass không panic.

### Xác nhận H-01:
- Chuyển hoàn toàn sang allowlist tĩnh `SAFE_OPEN_EXTENSIONS` cho thao tác mở trực tiếp.
- Đuôi file nguy hiểm hoặc ngoài allowlist chuyển sang `open_folder_and_select(path)` (reveal in Explorer).
- Vòng lặp cắt tỉa trailing dots/spaces và tiền tố hóa thiết bị DOS (`_CON.txt`) vô hiệu hóa triệt để bypass Win32.
- Gắn stream NTFS ADS `Zone.Identifier` với `ZoneId=3` (Mark-of-the-Web).

