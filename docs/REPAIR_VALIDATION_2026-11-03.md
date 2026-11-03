# Bản sửa sync, media và thông báo — 03/11/2026

Nền mã đã bao gồm hai commit mới của maintainer: `15344dd` và `c406019`. Giữ đồng bộ mute với tài khoản, RTF, profile, proxy, danh bạ và ô soạn nhiều dòng. Các HTTP client mới dùng cùng cấu hình proxy; dữ liệu/cache mới nằm dưới thư mục profile đang chạy. Không đổi schema 6.

## Các thay đổi

| Mục | Hành vi trong mã |
| --- | --- |
| 1 | Nhóm sync bảo thủ tối đa 30 partition mặc định; download xuống đĩa song song với giải mã/nhập có thứ tự, hàng đợi giới hạn và checkpoint. Hai dòng tải về/nhập CSDL. |
| 2 | Thumbnail trước, cache trước mạng, chỉ tải preview khi mở; ảnh gốc khi lưu. Bảng media bên phải cũng không lấy ảnh gốc thay thumbnail. |
| 3 | Video có thumbnail/nút play; tải theo luồng xuống tệp rồi phát trong cửa sổ MFPlay có âm thanh, pause, tua ±10 giây, âm lượng và fullscreen. Lần đầu phải chờ tải xong; chưa có phát progressive. |
| 4 | Lưu tệp/ảnh gốc/video bằng bộ chọn tệp; tiến độ, hủy, mở tệp/thư mục. Hai lượt truyền đồng thời, không giữ toàn bộ tệp trong RAM hoặc ghi đè tệp đích đã tồn tại. |
| 5 | Popup native topmost, không lấy focus, tên/avatar/preview, nút ×, tự đóng sau 4,5 giây; tin mới thay bằng hiệu ứng 150 ms, click mở đúng hội thoại. |
| 6 | Ẩn call log cũ/mới khỏi chat, preview, tìm kiếm và đếm chưa đọc. Giữ các bản ghi trong DB, không xóa dữ liệu. |
| 7 | Dùng chung formatter tên người gửi nhóm trong sidebar và popup. |
| 8 | Nhận/kết thúc ở thiết bị khác dọn trạng thái gọi đến; thao tác nhận/từ chối/kết thúc có kiểm tra mã cuộc gọi. |
| 9 | Vùng trạng thái sync cao cố định, nhãn không wrap; ID tương tác theo message và khung media ổn định. |
| 10 | Kiểm tra clipboard DIB/PNG và ngân sách pixel trước cấp phát; trả native handle trước giải mã; ghi chẩn đoán crash cục bộ không chứa nội dung chat/session. |
| 11 | Badge taskbar 1–5/5+, tray chấm đỏ; tin chat không yêu cầu nhấp nháy. Các cuộc gọi vẫn có cơ chế attention riêng. |

Âm thanh dùng MP3 công khai của Zalo, cache một lần và phát cục bộ, theo setting/mute. Nếu không tải được, app báo trạng thái; chưa thể nghe thử asset trong môi trường này do CDN trả lỗi truy cập.

## Xác minh

- 353 test tự động đã qua, 6 test thủ công bỏ qua trên Linux, gồm HTTP/SOCKS giả lập, clipboard parser, phân biệt chất lượng media, call filtering, checkpoint/final marker, popup replacement và bảo vệ tệp tải về.
- Clippy toàn bộ target trên host Linux.
- Build debug và smoke GUI qua Xvfb: ảnh/video/tệp, sync cửa sổ nhỏ, RTF, danh bạ. Dữ liệu giả, không tài khoản thật.
- Compile/Clippy các module Win32 mới qua harness ARM64 riêng: popup/tray, clipboard, crash diagnostics và MFPlay, gồm biên dịch bài test native popup. Đây không phải build/link toàn bộ sản phẩm Windows, cũng không phải kiểm thử chạy Windows.
- CI native Windows có bài test cửa sổ popup, smoke media mới và các bước build/PE/package hiện có.

## Cần kiểm tra trên bản Windows

1. Đăng nhập Android/iOS, đồng bộ nhiều hội thoại; kiểm tra tải/nhập tiến triển độc lập, dừng/mất mạng không mất dữ liệu đã nhập, bấm lại không trùng lịch sử.
2. Ảnh/video lần đầu và lần mở lại; file lớn; hủy khi mạng chậm; hết dung lượng; lưu ra ổ NTFS và ổ rời.
3. Minimize/tray khi nhận tin cá nhân/nhóm, mute từ thiết bị khác, bật/tắt setting và âm báo; click/đóng popup, thay tin liên tiếp, badge sau khi đọc trên thiết bị khác, Explorer restart.
4. Nhận/kết thúc cuộc gọi trên điện thoại khi app đang reo; cuộc gọi tiếp theo không bị thao tác từ bảng cũ ảnh hưởng; chat không hiện call log.
5. Paste từ Snipping Tool/Explorer, ảnh nhiều megapixel, nhiều tệp; nếu văng thì xem thư mục `diagnostics` của profile.

Windows SDK/MSVC hiện chưa tải được: đường tải Visual Studio trả HTTP 403 qua proxy môi trường. Bản `.exe` cần được build bởi workflow Windows ARM64 sau khi đưa mã đã duyệt lên branch. Không coi bản debug Linux hoặc API harness là bản chạy thử Windows.
