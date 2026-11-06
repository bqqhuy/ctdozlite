# Kiểm chứng v0.008 — ngày 06/11/2026

Working tree `feature/v008-arm`. Kết quả lệnh cuối được cập nhật trong
[tiến độ release](../../docs/releases/v0.008-arm-progress.md).

Regression: account/PIN scope, missing verifier, lockout, cache khác tài khoản,
relock/search generation, normal refresh không đóng search, alias cache/readback,
frame chat không giả làm control, control nested-array, activity không gây full refresh,
group aliases/revision/cached-ACL, numeric owner, poll/note identity/kind, archive UID trần,
QR/username/group report contracts và runtime generation không tái sử dụng.
GUI headless kiểm tra sáng/tối và cửa sổ nhỏ; không thay kiểm tra native/IME/font thật.

## Nghiệm thu còn cần thiết bị

1. Hai client thay alias/label/archive/manual unread; reconnect và mất ACK, đối soát trên client kia.
2. Đặt PIN test, ẩn direct/nhóm; mở khóa, khóa khi đang search/view/video/download/reaction/schedule.
   Đổi PIN từ client kia; app phải khóa trước khi tải lại. Restart offline: chat thường đọc được,
   chat ẩn vẫn khóa; không có PIN/verifier trong cache/dump. File đã mở ngoài app không bị đóng.
3. Username/QR/profile/avatar/history đúng UID; hủy picker, lỗi mạng, đổi account giữa tác vụ.
4. Group thường/cộng đồng: creator/admin/member; thay role/lock, kick/ban khi chat đang mở.
   Không có frame cache/hydrate nào làm hiện lại ô soạn được gửi khi quyền đã bị thu hồi.
5. Join/invite/pending/partial review/quota/chuyển trưởng; không báo đã tham gia từ ACK chờ duyệt.
6. Poll/note nhóm A không bị thao tác bằng quyền admin nhóm B; poll anonymous không suy UID.
7. Windows ARM64: font/emoji, DPI 100/150/200%, theme, viewer và tắt/mở lại ứng dụng.

Chưa chạy các tình huống này trên tài khoản/thiết bị thật. API có trong bundle không bảo đảm
mọi tài khoản đều có entitlement; trường thiếu hoặc bị từ chối vẫn phải báo rõ và giữ dữ liệu.

Lượt cuối 06/11: cargo test --locked 632 passed/0 failed/10 ignored; clippy all-targets
-D warnings, fmt --check và git diff --check qua. Không có xác nhận native/device mới.
