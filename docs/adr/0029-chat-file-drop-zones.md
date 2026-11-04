# ADR-0029: Hai vùng kéo thả tệp trong hội thoại

- Status: accepted
- Date: 2026-11-04
- Extends: ADR-0021, ADR-0027, ADR-0028.

## Decision

Thả vào viewport lịch sử chat gửi ngay; thả vào composer (gồm danh sách tệp chờ)
chỉ thêm tệp, người dùng xem tên/dung lượng/thumbnail ảnh, bỏ từng tệp, thêm chú thích
rồi bấm Gửi/Enter. Caption áp dụng cho từng tệp trong lượt gửi, như luồng chọn tệp
hiện có. Thả ngoài hai vùng hoặc khi modal/menu đang mở không gửi.

Overlay nêu rõ “Thả để gửi ngay” hoặc “Thả để đính kèm”. Chỉ xử lý drop ở layout pass
cuối, tránh gửi lặp khi egui bỏ một lượt layout. Vị trí không xác định không được
coi là vùng gửi. winit 0.30.13 bỏ tọa độ OLE DragOver trên Windows; lấy GetCursorPos
và ScreenToClient của HWND do eframe sở hữu, chia pixels_per_point để khớp DPI/zoom.
Trong lúc kéo, repaint mỗi 33 ms; kết thúc kéo trở lại cơ chế repaint thông thường.

Tệp chờ chỉ giữ trong RAM, tách tài khoản/thread; đổi hội thoại không chuyển tệp.
Đăng xuất/đổi tài khoản hoặc xóa hội thoại xóa danh sách tương ứng; kết quả preview
muộn bị bỏ qua theo owner/thread/id. Không lưu đường dẫn tệp vào bản nháp SQLite.
Giới hạn 16 tệp/thread, 64 tệp chờ tổng, tối đa 16 tệp/lần thả; khử trùng đường dẫn
trong cùng thread. Queue preview tối đa 64, hai tác vụ đọc/giải mã cùng lúc.

Worker đọc metadata và tạo thumbnail tối đa 64 px với giới hạn decode hiện có.
Ảnh lớn/không hỗ trợ và video/tệp khác dùng biểu tượng tệp, không cản gửi.
Tệp lỗi phải được bỏ rồi thả lại. FileStamp của bản xem trước đi cùng payload gửi;
tệp thay đổi bị từ chối trước upload. Không tự đổi sang nội dung mới mà người dùng
chưa xem. Thư mục/tệp không đọc được tạo lỗi, không mở luồng upload.

Dùng chung đường gửi attachment, HD/original, hàng đợi, clientId, Store và retry
hiện có. Session được chụp trước khi mở picker; lệnh gửi tệp kéo thả còn kiểm tra owner.
Mention, quote, ảnh clipboard và hẹn giờ chưa kết hợp với batch tệp chờ: báo rõ,
giữ danh sách để người dùng điều chỉnh. Không đổi protocol/schema/phiên bản đã phát hành.

## Validation

Test vùng gửi/compose/ngoài vùng/modal/không có tọa độ, giới hạn/khử trùng/cách ly
state, preview ảnh/tệp lỗi, kết quả muộn và từ chối tệp thay đổi trước mạng.
Kiểm tra API cursor bằng cargo check với target Windows ARM64. Cần thử kéo Explorer
trên Windows thật ở DPI 100%/150%/200%, nhiều màn hình, modal, đổi chat và logout.
Maintainer duyệt merge/build/release v0.005-arm; Cargo/Windows file version 0.0.6,
manifest 0.0.6.0. Phát hành artifact đã qua CI native ARM64 từ đúng commit main.
