# Tham chiếu Zalo PC cho sửa UI và đồng bộ

Nguồn: bộ cài do maintainer cung cấp, Zalo PC 26.11.10.3400. Chỉ giải nén tĩnh NSIS → app32.7z → app.asar; không chạy bộ cài. SHA-256 bộ cài: `7166ea28925b3121369ffb25e87b4749a648f47a861272009dcb14094b5a5511`; ASAR: `70f1b53c4db61321bf1b11c722a9049823711328b87e17b0d19338f83a5f6ed3`.

- Controller `transfer_sync2` / `upload_batch` phân biệt metadata, message và metadata có nhúng message. Cờ `isLast` đi cùng `scopes` phải được đối chiếu từng partition, kể cả batch metadata nhúng message. Không dùng cờ này để kết thúc mọi partition còn lại.
- Phản hồi lỗi của batch (`err`/`errorCode`) phải được nhận trước khi đợi hoàn tất; không biến lỗi giao thức thành timeout điện thoại.
- Cấu hình batch/concurrency của client chính thức có thể đến từ remote config. Không suy ra 30 là giới hạn bắt buộc của máy chủ hoặc ngưỡng gây khóa tài khoản. Nhóm 30 là lựa chọn tương thích bảo thủ của fork theo yêu cầu maintainer, có giảm tiếp khi gặp phản hồi giới hạn được nhận diện.
- Popup tham chiếu có vùng nội dung khoảng 350×80 logical pixels, avatar 52, tên 16 và preview 14; cửa sổ native topmost và không lấy focus. Fork dùng 4,5 giây theo yêu cầu, khác bộ đếm 10 giây trong nguồn đã xem.
- Taskbar overlay và ảnh tray là hai đường cập nhật riêng. Âm báo tham chiếu dùng tài nguyên công khai tại hằng `NOTIFICATION_SOUND` trong `src/endpoints.rs`; fork tải/cache khi bật âm thanh, không đưa tài nguyên trích xuất vào repository.

Tài nguyên giải nén và bản pretty-print nằm ngoài repository. Các kiểm thử dùng dữ liệu giả, không payload hay khóa của tài khoản thật. Chưa có capture từ một lượt đồng bộ điện thoại thật cho lần sửa này; cần kiểm tra Android/iOS với bản Windows chạy thử. Xem ADR-0014 và ADR-0015.
