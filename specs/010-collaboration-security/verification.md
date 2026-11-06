# Tiêu chí Nghiệm thu Khối R12 & R13 (Verification)

## 1. Unit Tests
- [x] `test_parse_todo_list`: Kiểm tra bóc tách to-do items từ JSON response Zalo.
- [x] `test_todo_overdue`: Kiểm tra tính toán hạn chót (overdue/deadline).
- [x] `test_parse_poll_item`: Bóc tách poll data, tính toán tỷ lệ phiếu bầu từng lựa chọn.
- [x] `test_quick_message_expansion`: Mở rộng phím tắt `/dc`, `/stk` và tìm kiếm tin nhắn nhanh.
- [x] `test_quick_message_upsert_and_remove`: Thêm/sửa/xoá tin nhắn nhanh.
- [x] `test_parse_device_list`: Bóc tách danh sách phiên thiết bị đăng nhập và nhận diện thiết bị hiện tại.

## 2. Giao diện & Tương tác
- [x] Tab "Giao việc & Bình chọn", "Tin nhắn nhanh", "Thiết bị & Bảo mật" trong Tools panel.
- [x] Gõ phím tắt bắt đầu bằng `/` trong khung chat tự động mở rộng thành nội dung tin nhắn mẫu.
- [x] Thẻ bong bóng To-do và Poll hiển thị nhất quán trên giao diện chat egui.
- [x] Khả năng đăng xuất từ xa thiết bị khác qua `WorkerCmd::KickDevice`.
