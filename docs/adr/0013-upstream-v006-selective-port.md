# ADR-0013: Tiếp nhận có chọn lọc upstream v0.0.6

- Status: accepted
- Date: 2026-11-02
- Reference: upstream ctdoteam1110/ctdozlite `9328cc9` (#67, #64), `ed550f6` (v0.0.6). Bằng chứng giao thức: `re-notes/13_mute_sync_and_rich_text.md`.

## Context

Maintainer yêu cầu tiếp nhận năm phần của upstream v0.0.6: đồng bộ tắt thông báo hai chiều, hiển thị tin định dạng, hồ sơ nhiều tài khoản, proxy và trang Danh bạ. Không merge nguyên commit: upstream lưu mật khẩu proxy trong `session.json`, chèn dấu Markdown vào nội dung tin, tự đổi `*`/`__` trong tin gửi đi thành định dạng và hiển thị tab "lời mời" luôn trống.

## Decision

**Tắt thông báo.** ADR-0004 coi tắt thông báo là lựa chọn trên máy này; quyết định này thay đổi riêng cờ đó thành trạng thái của tài khoản, ghim/ẩn vẫn cục bộ. Đổi trong menu gọi `setmute` trước, chỉ lưu khi máy chủ xác nhận (fail-closed). Mỗi lần làm mới hội thoại đọc ảnh chụp `getmute`; ảnh chụp đầy đủ là nguồn đúng cho cờ tắt thông báo. Sự kiện `act_type: "mute"` từ WebSocket hoặc `lp_control` cập nhật ngay. Một bộ đếm thế hệ bỏ ảnh chụp đang chạy nếu có thay đổi mới hơn. Lần đối soát đầu tiên của mỗi tài khoản (cờ `mute_sync_seeded` trong `tool_config`, không đổi schema) đẩy tối đa 50 hội thoại đã tắt cục bộ lên máy chủ thay vì bật lại chúng; chỉ đánh dấu xong khi đẩy hết. Tắt thông báo có hạn từ thiết bị khác được coi là đang tắt tới lần làm mới sau khi hết hạn.

**Tin định dạng.** Chỉ hiển thị tin nhận `action: "rtf"`. Nội dung chữ giữ nguyên; kiểu chữ lưu riêng trong `text_styles` (offset UTF-16, đã kiểm tra phạm vi). egui không có font đậm nên chữ đậm được vẽ lớn hơn 0,6px. Không chuyển Markdown khi gửi.

**Hồ sơ.** `--profile TÊN` chọn `<thư mục dữ liệu>/profiles/<tên>` với session, lịch sử, cài đặt và khóa instance riêng, nên hai hồ sơ chạy song song được. Tên chỉ gồm chữ/số ASCII, `-`, `_`, tối đa 32 ký tự, chuyển chữ thường; tên thiết bị dành riêng của Windows bị từ chối. Tên sai bị từ chối, không tự sửa.

**Proxy.** `--proxy URL` hoặc `CTDOZ_PROXY` (http, socks5, socks5h) áp dụng cho mọi HTTP client và cho WebSocket (HTTP CONNECT hoặc SOCKS5, rồi TLS như cũ). Cấu hình chỉ nằm trong bộ nhớ của tiến trình, không ghi xuống đĩa; log chỉ in URL đã che thông tin đăng nhập. Cuộc gọi dùng UDP nên không đi qua proxy. Proxy https:// chưa hỗ trợ.

**Danh bạ.** Trang riêng thay danh sách hội thoại khi mở, hai tab Bạn bè/Nhóm từ danh sách bạn bè và hội thoại đã tải, nhóm A–Z theo tên bỏ dấu, bỏ Cloud và hội thoại ẩn. Không có tab lời mời vì client không tải dữ liệu đó. Ô soạn tin nhiều dòng: Enter gửi, Shift+Enter xuống dòng, cao tối đa 5 dòng rồi cuộn; Enter chỉ bị chặn khi ô soạn tin đang có focus.

Không tiếp nhận: gói sticker GIF "Mèo Ami Bụng Bự" (chưa rõ bản quyền ảnh), giao diện hẹn giờ mới của upstream (fork đã có bộ hẹn giờ theo ADR-0012), đồng bộ điện thoại #68 (fork đã có ADR-0005/0007/0010).

## Verification / rollback

Test tự động (không mạng thật, không tài khoản thật): phân tích `getmute`/control/`setmute`, đối soát, giao dịch SQLite giữ ghim/ẩn và rollback; phân tích RTF, cắt đoạn theo UTF-16 với emoji; tên hồ sơ; proxy qua máy chủ HTTP/SOCKS5 giả trên loopback (CONNECT, xác thực Basic, 407, socks5h); nhóm A–Z; số dòng ô soạn. Ảnh chụp headless (Xvfb) trang Danh bạ, tin định dạng và ô soạn 3 dòng. Chưa kiểm chứng với máy chủ Zalo thật hay Windows ARM64 thật.

Không đổi schema (vẫn 6). Rollback bằng binary cũ an toàn: cột `is_muted` vẫn đọc được; cờ `mute_sync_seeded` bị bỏ qua; `text_styles` là trường serde tùy chọn; thư mục `profiles/` không bị bản cũ đụng tới.
