# ADR-0019: Công cụ dữ liệu và updater của fork

- Status: accepted
- Date: 2026-11-03
- Extends: ADR-0012 (updater exclusion superseded by maintainer approval); ADR-0004 local-only clear gains a separate server action.

## Decision

Nhập/Xuất lịch sử có entry trực tiếp ở Cài đặt/Đồng bộ. Export SQLite đọc snapshot, ghi streaming từng row vào file riêng chỉ gồm threads/messages và marker owner/version; không xuất config hoặc quyền tự động gửi. Xuất không ghi đè destination. Import xác minh owner marker nếu có; nguồn khác vẫn cần xác nhận ownership. Không tuyên bố hỗ trợ JSONL/schema lạ/backup PC mã hóa.

Xóa cuộc trò chuyện server là hành động xác nhận riêng với clear local. Cần client identity thật của tin cuối, API success rồi mới transaction xóa local; không invent cliMsgId theo thời gian. Không rời nhóm hoặc thu hồi tin của người khác. Mất phản hồi giữ local để tránh false success.

Updater thủ công chỉ đọc release ổn định mới hơn từ bqqhuy/ctdozlite. Bắt buộc checksum của installer và portable ARM64 đi kèm; xác minh PE ARM64 của portable. Inno bootstrapper không bắt buộc cùng kiến trúc payload. Download/stage bằng client không cookie; redirect allowlist GitHub; không thay running executable. Người dùng bấm mở bộ cài riêng, hash được kiểm tra lại trước launch và chỉ đóng app nếu launch thành công. Không query upstream hoặc tự mở installer khi kiểm tra cập nhật.

Bỏ timer repaint 1,2 giây; event/deadline animation tiếp tục yêu cầu repaint. Lookup avatar nhóm theo ID, không tên hiển thị.

## Verification and rollback

Test export/import round-trip, ownership/idempotency, checksum/architecture selection và PE. Cần kiểm tra updater với release fork thực và installer trên Windows ARM64. Không đổi release tag/version trong đợt này. Rollback UI không xóa dữ liệu người dùng.
