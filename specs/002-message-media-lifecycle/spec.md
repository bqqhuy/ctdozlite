# Vòng đời tin nhắn, media và reaction

Ngày: 2026-11-05 (Asia/Saigon). Phạm vi: v0.007-arm.
Trạng thái toàn tính năng: **một phần**; chưa xác nhận trên thiết bị.
Nguồn: yêu cầu đã được người dùng chốt; không đề xuất lại toàn bộ phạm vi.

## Mục tiêu và yêu cầu

| ID | Hành vi | Nghiệm thu |
| :--- | :--- | :--- |
| M-01 | Tin tự xóa dựa trên timestamp/TTL server, không sống lại sau import. | DB/FTS/UI không trả tin hết hạn; delete marker chặn replay; expiry được lọc trước LIMIT. |
| M-02 | Chặn chia sẻ được áp nhất quán cho tin có nguồn. | Forward/copy/save/export/library/viewer kiểm tra đúng thread; không có nhánh mở cache bỏ qua policy. |
| M-03 | Media hết hạn không xuất hiện lại từ tác vụ cũ. | Ảnh queued sau expiry/account switch bị bỏ; video dừng; cache app sở hữu được dọn, không xóa file người dùng đã lưu. |
| M-04 | Reaction qua nút, chi tiết người thả và preview không tạo unread giả. | Nhiều target cùng đợt đều cập nhật; clear bỏ đúng reaction; target hết hạn không giữ preview cũ. |
| M-05 | Ảnh phục hồi cục bộ được decode có giới hạn. | JXL đọc không cần URL; ảnh nhỏ không bị phóng; input hỏng/quá lớn bị từ chối. |

## Bằng chứng và giới hạn

Module nền: `src/message_lifecycle.rs, src/conversation_policy.rs, src/reaction_pipeline.rs, src/desktop_backup.rs, src/gui/worker.rs`.
[Hợp đồng PC/Web](../../re-notes/31-v007-contracts.md),
[ADR-0031](../../docs/adr/0031-v007-group-policy-and-restoration.md).
Bộ kiểm thử cục bộ chưa chứng minh đầy đủ cleanup cache thường/native Windows; danh sách reactor chỉ gồm UID thật đã nhận.

## Tình huống thiết bị

1. Mở ảnh/video sắp hết hạn, đổi tài khoản trong khi tải và quay lại hội thoại; xác nhận không tái hiện nội dung cũ.
2. Bật chặn chia sẻ trên Zalo chính thức; thử copy/forward/save/library/viewer của fork rồi thử lại khi tắt.
3. Hai tài khoản thả/bỏ nhiều loại reaction; kiểm tra preview, danh sách tên/avatar và unread khi target đến muộn.

## Ngoài phạm vi

Không thêm API phỏng đoán, chống thu hồi, telemetry hoặc tự động phát hành.
Spec này không mở rộng ủy quyền thao tác tài khoản thật; nghiệm thu cần môi trường
và thao tác thử đã được người dùng cho phép.
