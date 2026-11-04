# ADR-0028: Chọn lọc cải tiến upstream cho v0.004-arm

- Status: accepted
- Date: 2026-11-04
- Extends: ADR-0021, ADR-0024, ADR-0027.

## Decision

Giữ nền fork; tham khảo PR #78 và #85, không nhập toàn bộ các PR phân kỳ.
Menu chuột phải áp dụng cả hàng tin nhắn; album giữ đúng từng ảnh. Thanh cảm xúc
hover dùng nút egui và định danh tin ổn định, gửi qua mô hình reaction hiện có.
Có nút về tin mới nhất khi đang đọc lịch sử; không ép cuộn xuống khi nhận tin.

Mention nhóm giữ UID đã chọn và vị trí UTF-16, gửi `mentionInfo` theo bằng chứng
trong re-notes/28. Gợi ý lấy từ danh sách thành viên có UID thật, hỗ trợ @All,
chọn bằng chuột hoặc Enter dòng đầu. Token có UID được tô màu/gạch chân; chữ @tên
chưa chọn vẫn là văn bản thường. IME không dùng Enter vừa chọn vừa gửi.
Chỉnh sửa chạm vào token xóa danh tính của token đó; không suy UID từ tên hiển thị.
Retry lưu đủ mention và clientId. Tin có mention và link gửi dạng text để giữ UID.
Hẹn giờ và caption ảnh chưa hỗ trợ mention: báo rõ, giữ nội dung để gửi riêng.

Bản nháp lưu chữ nguyên văn trong composer_drafts; metadata có ràng buộc nguyên văn
trong tool_config của tài khoản, cập nhật trong cùng transaction, không đổi schema.
Không đọc metadata từ nội dung người dùng dán vào editor. Xóa hội thoại xóa cả metadata.

Upload chấp nhận thêm dạng control trực tiếp/mảng và fileUrl, có giới hạn duyệt,
không nhận fileId -1 hoặc 0. Giữ registry theo tài khoản. Chờ URL tối đa 90 giây;
không gửi tệp thiếu URL và không tự tải lại video nếu máy chủ chưa xử lý xong.

Release label v0.004-arm; phiên bản Cargo/Windows là 0.0.5 (manifest 0.0.5.0).
Build và phát hành từ main sau khi CI Windows ARM64 thành công, theo yêu cầu
của maintainer. Không đổi luồng đồng bộ điện thoại.

## Validation

Test giao thức bằng máy chủ loopback giả lập, Unicode/IME và lifecycle bản nháp,
parser upload có giới hạn; chạy cargo test và Clippy. Cần kiểm tra mention nhận
trên tài khoản thật và tương tác GUI trên Windows ARM64 trước phát hành.
