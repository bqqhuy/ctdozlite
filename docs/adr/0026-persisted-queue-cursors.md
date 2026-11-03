# ADR-0026: Lưu con trỏ hàng đợi qua các lần mở app

- Status: accepted
- Date: 2026-11-03
- Evidence: `re-notes/20-queue-cursor-persistence.md`. Builds on `4076ef4` (`re-notes/19-branch-audit-pc-web.md`), which already made server-history backfill explicit and durable.

## Context

Sau `4076ef4`, lịch sử máy chủ không còn tự chạy và đã có mốc hoàn tất. Nhưng mỗi lần mở app (và mỗi lần kết nối lại), app vẫn kéo `lp_chat` từ `lastActionId = 1` và mọi hàng đợi tin trên WebSocket từ `lastId = 1`. Máy chủ trả lại các trang tin đã có: kết quả nhập bằng 0 nhưng vẫn tốn thời gian. Zalo PC lưu con trỏ theo tài khoản và tiếp tục từ đó.

## Decision

- Lưu con trỏ `lp_chat` và con trỏ các hàng đợi tin 510/511/513–518 theo tài khoản, đọc lại ở lần poll đầu và mỗi lần kéo hàng đợi (cả khi kết nối lại). Chỉ ghi con trỏ khi trang đó đã vào SQLite.
- Khi một lần poll trong cùng chu kỳ bị lỗi, các trang nhận trước đó vẫn được lưu (trước đây bị bỏ).
- Đồng bộ điện thoại không đổi: lượt cập nhật vẫn cần điện thoại gửi danh sách hội thoại, rồi chỉ xin các hội thoại có `lastTs` vượt quá phần đã phủ (ADR-0024).

## Verification / rollback

Test tự động: lệnh kéo đầu tiên dùng con trỏ đã lưu; so sánh ID dạng số; ghi tệp chỉ khi có thay đổi và có giới hạn tần suất. Cần thử thật: mở lại app sau khi đã đồng bộ, lượt bắt kịp phải ngắn và không tải lại trang cũ.

Rollback an toàn: bản cũ bỏ qua tệp `*.queue-cursors.json` và kéo lại từ đầu như trước.
