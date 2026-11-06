# Trạng thái tài khoản và nhóm — v0.008-arm

Ngày: 06/11/2026 (Asia/Saigon). Phạm vi R9/R10 đã được người dùng cho triển khai.
Mã đã được bổ sung trên `feature/v008-arm`; nghiệm thu thiết bị được ghi riêng.

| ID | Yêu cầu |
| :--- | :--- |
| A-01 | Biệt danh, nhãn, lưu trữ và dấu chưa đọc là state server riêng; có snapshot/readback, không lặp mutation khi ACK không rõ. |
| A-02 | Chat ẩn có PIN và danh sách theo tài khoản; không lẫn local hide; khóa lại chặn sidebar/search/viewer/download/notice/cửa sổ phụ. |
| A-03 | Cache offline không chứa PIN/verifier; chỉ hỗ trợ chat thường và giữ chat ẩn khóa tới khi có xác minh online. Cache hỏng/sai chủ không mở khóa. |
| A-04 | Profile/avatar/history, QR và username dùng worker, UID thực, xác nhận mutation và account/generation guards. |
| A-05 | Presence theo privacy; trạng thái chưa có dữ liệu không được vẽ thành online/offline giả. |
| G-06 | Role/membership/setting control vô hiệu hóa quyền trước khi tải lại; phản hồi cũ/cache đĩa không phục hồi quyền gửi. |
| G-07 | Rename/avatar/settings/link/QR/invitation/pending/ban/community/quota/chuyển trưởng dùng đúng khả năng và quyền server. |
| G-08 | Poll/ghi chú cần liên hệ với đúng bảng tin nhóm; quyền sửa/xóa không dựa vào `creator` do UI truyền lên. |

Không tuyên bố PIN mã hóa SQLite/file hoặc đóng được file đã mở bằng ứng dụng ngoài.
Ghi bạn thân không bật: PC gốc tắt feature, hàm toggle rỗng và worker bỏ qua action;
chưa xác minh enum ghi hợp lệ. Vẫn có API đọc danh sách cũ và UI tra cứu.
Các chức năng poll/voter/rich-note nâng cao tiếp tục thuộc R12/v0.009 theo roadmap.
