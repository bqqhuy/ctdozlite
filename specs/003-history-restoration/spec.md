# Khôi phục lịch sử V1 và backup Desktop V2

Ngày: 2026-11-05 (Asia/Saigon). Phạm vi: v0.007-arm.
Trạng thái toàn tính năng: **một phần**; chưa xác nhận trên thiết bị.
Nguồn: yêu cầu đã được người dùng chốt; không đề xuất lại toàn bộ phạm vi.

## Mục tiêu và yêu cầu

| ID | Hành vi | Nghiệm thu |
| :--- | :--- | :--- |
| H-01 | V1 chỉ chạy do người dùng chọn, không tự thay Sync2. | Phiên match public key/computer; không ghi coverage Sync2; lỗi không tự kích hoạt luồng khác. |
| H-02 | V1 tải/giải mã/giải nén/nhập có giới hạn và hủy được. | XZ budget 128 MiB trước cấp phát; tổng output có cap; hủy/đổi tài khoản dừng; không log khóa/URL. |
| H-03 | V1 giữ đúng identity và metadata tin cũ. | UID ánh xạ qua znoise, không hardcode self; BinNet chưa decode phải ghi giới hạn, không báo media đầy đủ. |
| H-04 | Desktop V2 có dry-run/account binding và import lặp an toàn. | Sai tài khoản/key/file thay đổi không nhập âm thầm; giữ delete marker; hủy rollback lô hiện tại, giữ lô đã commit. |
| H-05 | Media backup bền vững và độc lập cache có thể xóa. | Đúng attachment/chất lượng được gắn đúng tin; JXL local mở được; cache cleanup không xóa asset còn tham chiếu. |

## Bằng chứng và giới hạn

Module nền: `src/phone_sync/legacy.rs, src/phone_sync/assets.rs, src/desktop_backup.rs, src/desktop_backup_format.rs, tests/desktop_backup_v2.rs`.
[Hợp đồng PC/Web](../../re-notes/31-v007-contracts.md),
[ADR-0031](../../docs/adr/0031-v007-group-policy-and-restoration.md).
V1 chưa nghiệm thu iPhone và chưa decode đủ BinNet. CBC/tar hợp lệ không phải bằng chứng xác thực mật mã của file backup.

## Tình huống thiết bị

1. Chạy V1 chủ động với iPhone: kiểm tra tin cũ chỉ còn trên điện thoại, hủy và chạy lại; xác nhận không đổi coverage Sync2.
2. Chọn backup của đúng tài khoản, dry-run rồi nhập; đối chiếu một tin và ảnh/video cụ thể trên Zalo PC chính thức.
3. Nhập lại, hủy ở giữa, đóng/mở ứng dụng; dữ liệu đã nhập còn đúng và tin đã xóa không sống lại.

## Ngoài phạm vi

Không thêm API phỏng đoán, chống thu hồi, telemetry hoặc tự động phát hành.
Spec này không mở rộng ủy quyền thao tác tài khoản thật; nghiệm thu cần môi trường
và thao tác thử đã được người dùng cho phép.
