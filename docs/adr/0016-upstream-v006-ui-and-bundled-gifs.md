# ADR-0016: Bổ sung UI và GIF từ upstream ed550f6

- Status: accepted
- Date: 2026-11-03
- Supersedes: phần không tiếp nhận GIF và giao diện hẹn giờ trong ADR-0013, theo yêu cầu bổ sung commit `ed550f6` của maintainer.
- Reference: upstream `ed550f6b5a69cbe154d68c28874104d7368d36bf`; các phần RTF, Danh bạ và ô soạn nhiều dòng đã có trong `15344dd`.

## Context

Maintainer yêu cầu bổ sung commit upstream và kiểm tra lại các issue/PR cụ thể. Fork đã có scheduler bền vững, ownership tài khoản, trạng thái lỗi/retry và API media streaming; cần giữ các đường này khi nhận UI/tài nguyên mới.

## Decision

Nhận 15 cặp thumbnail PNG/GIF Ami và nhãn từ upstream, tích hợp bộ chọn, mục phổ biến và gợi ý. ID/category của gói là định danh cục bộ. Tải thumbnail từ bytes nhúng trên blocking worker, không gọi CDN cho gói này. Khi gửi/chuyển tiếp, ghi GIF vào thư mục riêng của tài khoản bằng atomic write rồi dùng `send_attachment_with_client_id`; giữ nguyên client ID và chỉ chấp nhận thành công khi upload/send thật thành công. API sticker thông thường từ chối category cục bộ. Preview của gói là PNG tĩnh; không tuyên bố đã có trình phát mọi GIF inline.

Bổ sung hộp hẹn giờ có backdrop, bốn mốc nhanh, giờ OS, nội dung chỉnh sửa và chỉ dẫn rõ đây là lịch gửi trên máy này. Hộp giữ recipient/owner đã chọn lúc mở; đổi hội thoại không chuyển đích, đổi tài khoản làm vô hiệu draft. Dùng scheduler SQLite hiện có, không thêm scheduler song song hoặc báo lịch đã lưu trước phản hồi worker. Acknowledgement có request ID riêng; chỉ xóa ô soạn khi đúng yêu cầu đã được ghi thành công, lỗi giữ draft để thử lại. Nút đồng hồ/chuột phải nút Gửi mở hộp; bảng công cụ cũ vẫn quản lý/sửa/hủy lịch.

Giữ ô soạn tối đa năm dòng, ID editor theo thread, Enter/Shift+Enter và debounce sticker hiện có. Các nút được giữ trên hàng trên khi editor tăng chiều cao. Lưu trạng thái bảng thông tin hội thoại qua worker, giữ mặc định của fork cho cài đặt cũ.

Giữ phần RTF hiển thị có kiểm tra offset UTF-16; không tự chuyển dấu Markdown trong nội dung gửi. Danh bạ giữ hai tab có dữ liệu, không thêm tab lời mời trống. Không thay phiên bản package của fork hoặc số đo hiệu năng bằng metadata của release upstream.

## Verification / rollback

Test tài nguyên nhúng, đường thumbnail không mạng/không ghi cache riêng, chặn category giả trên API sticker, draft owner/recipient/literal text và thời gian không hợp lệ. Chạy suite/Clippy, build debug, smoke GUI hộp hẹn giờ lớn/nhỏ và bộ chọn GIF; CI Windows ARM64 bổ sung ba trường hợp đó.

Chưa xác minh gửi GIF thật hoặc UniKey trên Windows tương tác. Không đổi schema/dependency. Revert phần bổ sung không xóa lịch/thiết lập khác; các job retry GIF cục bộ cần adapter attachment của bản này.
