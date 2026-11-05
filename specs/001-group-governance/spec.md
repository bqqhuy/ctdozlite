# Quyền và quản trị nhóm

Ngày: 2026-11-05 (Asia/Saigon). Phạm vi: v0.007-arm.
Trạng thái toàn tính năng: **một phần**; chưa xác nhận trên thiết bị.
Nguồn: yêu cầu đã được người dùng chốt; không đề xuất lại toàn bộ phạm vi.

## Mục tiêu và yêu cầu

| ID | Hành vi | Nghiệm thu |
| :--- | :--- | :--- |
| G-01 | Composer phản ánh quyền gửi; đường gửi chung vẫn kiểm tra quyền server. | Nhóm khóa: thành viên thường không gửi được; trưởng/quản trị chỉ được gửi khi quyền xác nhận cho phép. |
| G-02 | Tạo nhóm, thêm/xóa thành viên, đổi quản trị, chuyển trưởng, rời/giải tán là các thao tác riêng. | ACK rõ ràng mới báo thành công; mất ACK không tự lặp mutation; không nhầm giải tán với xóa cục bộ. |
| G-03 | Thay đổi quyền/membership được áp vào nhóm đang mở. | Control hợp lệ làm mất hiệu lực cache đúng tài khoản/nhóm; không dùng roster cắt ngắn để suy ra quyền. |
| G-04 | Ghi chú và bình chọn dùng đúng loại, phân trang và quyền. | Đóng/bỏ phiếu/thêm lựa chọn/sửa ghi chú theo quyền; đếm phiếu ẩn danh không suy ra UID. |
| G-05 | Thông tin cá nhân liệt kê nhóm chung theo danh tính chuẩn. | ID 123/g123 không tạo hai nhóm; kết quả tài khoản cũ không mở hội thoại mới. |

## Bằng chứng và giới hạn

Module nền: `src/group_state.rs, src/groups.rs, src/group_content.rs, src/gui/group_panel.rs, src/gui/content_panel.rs`.
[Hợp đồng PC/Web](../../re-notes/31-v007-contracts.md),
[ADR-0031](../../docs/adr/0031-v007-group-policy-and-restoration.md).
Chưa xác nhận control tức thời và đầy đủ mô hình quyền cộng đồng; không coi lockSendMsg là toàn bộ mô hình quyền.

## Tình huống thiết bị

1. Thử nhóm thường/khóa/cộng đồng với vai trò thành viên, quản trị và trưởng nhóm; đổi quyền khi cửa sổ đang mở.
2. Tạo nhóm thử; thêm/xóa/chuyển quyền/rời/giải tán, kiểm tra trên Zalo chính thức sau từng ACK.
3. Bình chọn/ghi chú và nhóm chung trên dữ liệu thử, gồm phiếu ẩn danh, nhóm đã rời và phân trang.

## Ngoài phạm vi

Không thêm API phỏng đoán, chống thu hồi, telemetry hoặc tự động phát hành.
Spec này không mở rộng ủy quyền thao tác tài khoản thật; nghiệm thu cần môi trường
và thao tác thử đã được người dùng cho phép.
