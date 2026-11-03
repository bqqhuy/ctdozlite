# ADR-0024: Đồng bộ cập nhật, phục hồi media và các tính năng chọn lọc từ PR #78

- Status: accepted
- Date: 2026-11-03
- Builds on: ADR-0023.
- Evidence: `re-notes/15-phone-lifecycle-and-renewlink.md` (mục renewlink), `re-notes/18-typing-receipts-and-pr78.md`.

## Context

Log thực tế cho thấy: `renewlink` trả mã 0 nhưng app không đọc được kết quả; mỗi lần đồng bộ điện thoại vẫn xin lại mọi hội thoại; lượt Idle chờ đủ 600 giây dù điện thoại đang gửi lượt khác; log tóm tắt lượt ghi mỗi giây làm mất phần đầu log. "Tải từ máy chủ" bị hiểu nhầm là tải được toàn bộ lịch sử. Maintainer cũng yêu cầu tiếp nhận có chọn lọc PR #78 của upstream (`4a052c6`), trừ chống thu hồi.

## Decision

**Đợt 1: ổn định**

- **renewlink:** đọc từng lớp phản hồi (đối tượng hoặc chuỗi JSON), mã lỗi dạng số hoặc chuỗi, các trường `normal`/`hd`/`thumb`/`fileUrl` của Zalo PC và các trường `*Url` cũ; ảnh thu nhỏ video dạng chuỗi. Chỉ xin lại khi có cờ `async` (2/4/8 giây trong một hạn 15 giây tính cả thời gian mạng), vẫn qua cơ chế gộp yêu cầu. Log ghi lý do lỗi đã phân loại, không ghi URL.
- **Log:** tóm tắt lượt chỉ ghi khi số liệu đổi hoặc mỗi 10 giây; xoay vòng 2 × 4 MiB; tổng kết mỗi lần đồng bộ (đọc / mới / đã có / chưa nhận diện / lượt bỏ qua) và kế hoạch (cần kiểm tra / không đổi / tạm hoãn).
- **Đồng bộ cập nhật:** bỏ qua hội thoại khi coverage đã phủ tới mốc khoảng thời gian và tới `lastTs`; các hội thoại khác dừng ở `lastTs` + 2 phút. `lastTs` thiếu, bằng 0 hoặc lớn hơn hiện tại quá 1 ngày thì xin như cũ. Hội thoại chưa nhận diện được tạm hoãn 3 ngày trừ khi `lastTs` đổi; trạng thái này nằm trong tệp riêng của tài khoản (ghi nguyên tử; tệp hỏng coi như trống), không phải coverage; "Quét lại toàn bộ" xóa nó.
- **Lượt Idle:** Idle 60 giây trong khi lượt khác cùng lần đồng bộ nhận gói thì đánh thức một lần; 60 giây sau vẫn Idle thì kết thúc là đứng (dọn phiên, xin lại một lần). Điều kiện hoàn tất giữ nguyên.
- **Máy chủ:** đổi nhãn thành "Tải dữ liệu máy chủ còn cung cấp", nói rõ lịch sử cũ và chat cá nhân cần đồng bộ điện thoại. Hội thoại trả `isFiltered` được bỏ qua 24 giờ theo từng phiên đăng nhập; nút "Thử lại hội thoại bị giới hạn" bỏ qua bộ nhớ này. `isFiltered` không bao giờ được tính là lịch sử đầy đủ.

**Đợt 2: tính năng từ PR #78**

- **Đang soạn tin / đã nhận / đã xem:** nhận `actions` typing/gtyping và `delivereds`/`seens`/`groupSeens` từ WebSocket và `lp_chat`. Biên nhận chỉ nâng trạng thái tin gửi đi. Gửi typing theo `sendIsTyping` của Zalo PC (chat domain hoặc group domain, tối đa 10 giây/lần, không gửi cho Cloud), không theo cách của PR (host conversation ghi cứng, thêm `status`). Có công tắc cho cả hai chiều. Hành vi gửi "đã xem" không đổi.
- **Nhóm:** ngăn thông tin hiện thành viên (trưởng/phó nhóm trước, tìm không dấu, tổng số từ máy chủ, tên và avatar qua `getminiprofiles`, tối đa 1000). Mention tô màu theo `mentions[]` (offset UTF-16, phải bắt đầu bằng `@`). Avatar cắt vuông ở giữa rồi vẽ tròn.
- **Ảnh:** tùy chọn gửi bản HD (cạnh dài 2560 px, JPEG q90, xoay theo EXIF, GIF giữ nguyên); mặc định vẫn gửi ảnh gốc như trước. Request không đổi, chỉ đổi tệp tải lên. Không gửi các cờ `isHD`/`isOriginal` của PR vì chưa có bằng chứng.
- **Chuyển tiếp media:** ảnh, video, tệp được tải về thư mục riêng, gửi lại qua luồng gửi tệp sẵn có rồi xóa bản sao. Gọi trực tiếp `forward_message` với media vẫn bị từ chối.
- **Chụp màn hình (Windows):** Ctrl+Alt+S hoặc menu chuột phải ô soạn tin mở `ms-screenclip:`; app chờ tối đa 60 giây đến khi bộ nhớ tạm đổi, rồi đưa ảnh vào khung xem trước. Người dùng vẫn phải bấm gửi.
- **Không tiếp nhận:** chống thu hồi.

## Verification / rollback

Test tự động cho từng phần: các dạng phản hồi `renewlink` và hạn chót chung; tần suất ghi log; kế hoạch cập nhật và trạng thái tạm hoãn (kể cả tệp hỏng); quyết định Idle; bộ nhớ lọc theo phiên; phân tích typing và biên nhận; cập nhật trạng thái tin trong SQLite; offset mention; cắt avatar; thu nhỏ ảnh HD; điều kiện chuyển tiếp media. Mã chỉ chạy trên Windows (chụp màn hình) được kiểm tra biên dịch riêng cho `aarch64-pc-windows-msvc`. Cần thử trên máy thật: renewlink với ảnh đang lỗi, đồng bộ cập nhật lần thứ hai, typing/đã xem với tài khoản thật, chụp màn hình trên Windows 11 ARM64.

Không đổi schema SQLite. Tệp mới `*.sync-deferred.json` và `*.server-filtered.json` có thể xóa an toàn; bản cũ bỏ qua chúng. Cài đặt mới có giá trị mặc định qua `serde(default)`.
