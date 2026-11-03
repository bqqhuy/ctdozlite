# ADR-0025: Gửi video và chuyển tiếp media như Zalo PC

- Status: accepted
- Date: 2026-11-03
- Supersedes: phần "chuyển tiếp media bằng tải về rồi gửi lại" của ADR-0024 (nay là phương án dự phòng).
- Evidence: `re-notes/18-typing-receipts-and-pr78.md` (mục "Video messages and media forwarding like Zalo PC").

## Context

Fork gửi video như một tệp (`asyncfile` với `fType: 2`), nên bên nhận có thể chỉ thấy tệp. Chuyển tiếp media trước đây tải media về rồi gửi lại: chậm, tốn băng thông và hỏng khi link đã hết hạn. Zalo PC dùng đường riêng cho cả hai việc này.

## Decision

- **Gửi video:** tệp MP4/MOV/M4V được tải lên `video/file/upload`, chờ `video_done` để nhận URL, tải ảnh thu nhỏ (khung đầu, cạnh dài 720 px, lấy bằng Media Foundation trên Windows) lên `video/thumb/upload`, rồi gửi `/api/message/forward` hoặc `/api/group/forward` với `msgType: 5` và `msgInfo {videoUrl, thumbUrl, duration, width, height, fileSize, properties, title}`. Kích thước và thời lượng đọc từ hộp `mvhd`/`tkhd` của MP4 (thuần Rust, có giới hạn). Định dạng khác vẫn gửi như tệp. Nếu lỗi trước khi gửi tin video thì gửi như tệp, nên không có tin nào bị gửi hai lần.
- **Chuyển tiếp:** dựng `msgInfo` từ tin gốc giống Zalo PC; nhờ máy chủ sao chép media cho hội thoại đích bằng `renewlink` (hội thoại đích, client ID mới; video sao chép riêng tệp và ảnh thu nhỏ); rồi gửi `forward` với `msgType` 1/2/5. Nếu không làm được (tệp thiếu checksum, máy chủ từ chối sao chép) thì tải về rồi gửi lại. Khi link gốc hết hạn, hội thoại nguồn được lấy từ tin, hoặc từ CSDL khi mã tin chỉ có ở một hội thoại.
- **Bản sao ảnh HD:** tin lưu đường dẫn ảnh gốc (trường tùy chọn, tin cũ vẫn đọc được). Khi gửi lại mà bản sao đã bị dọn thì tạo lại từ ảnh gốc nếu ảnh gốc không đổi. Thư mục tách theo tài khoản; dọn khi app khởi động và sau mỗi lần tạo bản sao (quá 7 ngày, hoặc tổng quá 500 MB thì xóa bản cũ nhất); xóa khi đăng xuất. Gửi thành công thì xóa ngay.

## Verification / rollback

Test tự động: đọc MP4 phiên bản 0 và 1 (kể cả tệp hỏng), chỉ nhóm MP4 được gửi như video, `msgInfo` cho ảnh/video/tệp (tệp thiếu checksum bị từ chối), thân request `forward` cho 1-1 và nhóm qua máy chủ giả trên loopback, giới hạn dung lượng và tuổi của bản sao HD. Mã chỉ chạy trên Windows được kiểm tra bằng clippy cho `aarch64-pc-windows-msvc`. Cần thử với tài khoản thật: gửi video MP4 (bên nhận phát được ngay), chuyển tiếp ảnh, video và tệp sang 1-1, nhóm và Cloud, và chuyển tiếp tin cũ có link đã hết hạn.

Không đổi schema SQLite. Rollback về bản trước: tin gửi lại vẫn đọc được vì trường mới là tùy chọn; video lại được gửi như tệp.
