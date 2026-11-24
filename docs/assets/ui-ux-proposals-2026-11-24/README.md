# Minh họa hướng UI/UX — 24/11/2026

Ba ảnh sinh bằng AI để thảo luận bố cục; không phải screenshot ứng dụng, bằng chứng Zalo gốc hoặc thiết kế đã nghiệm thu.

- `01-composer-attachments-dark.png`: preview và chú thích trong cùng composer; bỏ từng tệp/thêm tệp/Gửi. Đây là ý định bố cục chính cần thực hiện.
- `02-workspace-light.png`: quan hệ bốn vùng, composer tích hợp và khoảng đệm. Banner là ví dụ state suggestion; vị trí, copy và kích thước cần đối chiếu PC trước khi chốt.
- `03-menu-and-settings-dark.png`: menu nhanh không có backdrop; modal Settings có nền chặn nhưng nội dung vẫn nhận input. Ảnh không chứng minh cơ chế input đã được sửa.

Các ảnh có khác biệt chi tiết (kiểu card file, thứ tự toolbar, nav, action drawer). Không lấy các khác biệt ngẫu nhiên này làm yêu cầu chức năng mới hoặc sao chép các nút chưa có capability. Giữ nav/actions và thiết kế hiện hữu ngoài phạm vi được giao; không thêm Nhật ký, đổi menu, chọn thứ tự toolbar hoặc gán quyền chỉ vì ảnh có vẽ. Không triển khai đồng thời hai hệ widget từ các ảnh.

Ảnh composer phóng lớn để dễ xem, không quy định chiều cao composer chiếm tỷ lệ đó khi chạy. Khi triển khai đo content bounds, giới hạn chiều cao khay và xử lý nhiều tệp/resize/DPI theo quy chế UI/UX. Tên dài được ellipsis có chủ đích và xem đầy đủ được; clip mất glyph/che nút là lỗi. Lựa chọn grid tile hay compact card cần thống nhất sau đối chiếu PC/Web và ngân sách chiều rộng.

Nguồn chuẩn: [kế hoạch V2](../../research/2026-11-24-zalo-full-parity-master-implementation-plan.md), [quy chế UI/UX](../../standards/ui-ux-consistency.md). Không có mã GUI mới trong lượt tạo minh họa.
