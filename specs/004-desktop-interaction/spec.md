# Hiển thị, flicker và điều hướng hội thoại

Ngày: 2026-11-05 (Asia/Saigon). Phạm vi: v0.007-arm.
Trạng thái toàn tính năng: **một phần**; chưa xác nhận trên thiết bị.
Nguồn: yêu cầu đã được người dùng chốt; không đề xuất lại toàn bộ phạm vi.

## Mục tiêu và yêu cầu

| ID | Hành vi | Nghiệm thu |
| :--- | :--- | :--- |
| U-01 | Chữ và trạng thái tương tác đọc được ở light/dark theme. | Popup, composer, disabled/hover/focus, thông báo sync và viewer được kiểm tra trên nền tương ứng. |
| U-02 | Tên có emoji/dấu tổ hợp được giữ khi hiển thị/cắt. | Không cắt giữa grapheme; kiểm tra glyph/ZWJ thực tế trên Windows ARM64, không suy từ chuỗi Unicode. |
| U-03 | Cập nhật hội thoại khác không làm ô chat nhấp nháy hay tải lại lịch sử vô ích. | Geometry chỉ đổi khi cần; metadata cùng version/head còn mới được tái dùng; invalidation vẫn lấy tin mới. |
| U-04 | Mute và bộ lọc chưa đọc nhất quán theo tài khoản. | Kết hợp Cá nhân/Nhóm/chưa đọc; bật/tắt mute và đọc từ thiết bị khác không tạo badge/thông báo sai. |

## Bằng chứng và giới hạn

Module nền: `src/gui/theme.rs, src/gui/workspace.rs, src/gui.rs, src/group_metadata.rs, src/history_open.rs`.
[Hợp đồng PC/Web](../../re-notes/31-v007-contracts.md),
[ADR-0031](../../docs/adr/0031-v007-group-policy-and-restoration.md).
Test tương phản và grapheme không thay kiểm chứng glyph thực tế hay quan sát flicker trên GPU Windows ARM64.

## Tình huống thiết bị

1. Windows ARM64: đổi sáng/tối khi settings/modal/viewer đang mở; rà chữ trắng trên nền sáng, hover và disabled.
2. Nhóm đông: cuộn, nhận tin ở nhóm khác, đổi hội thoại nhiều lần; ghi số request và hiện tượng flicker.
3. Tên có emoji ZWJ/cờ/dấu tiếng Việt; mute cá nhân/nhóm, lọc chưa đọc và đổi tài khoản.

## Ngoài phạm vi

Không thêm API phỏng đoán, chống thu hồi, telemetry hoặc tự động phát hành.
Spec này không mở rộng ủy quyền thao tác tài khoản thật; nghiệm thu cần môi trường
và thao tác thử đã được người dùng cho phép.
