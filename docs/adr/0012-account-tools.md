# ADR-0012: Tiện ích theo tài khoản và schema 6

- Status: accepted
- Date: 2026-11-02
- Reference: upstream ctdoteam1110/ctdozlite `8e164642ec7130be85466e0b30de586fa471d311`.

## Context

Maintainer yêu cầu tiếp nhận mục 1–7: candidate hosts, tìm SQLite, import lịch sử, theme JSON, sticker JSON, scheduler và auto-reply. Không nhập updater/REST/MCP, Feed giả dữ liệu, đường media x64 hoặc cơ chế gửi lại mutation của upstream. Các tiện ích phải dùng worker/session/store và định danh tin của fork.

## Decision

Candidate hosts đã xác minh được gom từ mọi mục service map và protected session. Sanitization chỉ giữ origin HTTPS thuộc Zalo, tối đa 16 origin/dịch vụ; host thành công được nhớ trong phiên qua lock ngắn dùng chung các snapshot. GET có thể failover trên connect/timeout/404; POST chỉ trên 404 chắc chắn, không tự replay sau mất phản hồi. Không mở failover chung cho multipart.

Schema 6 bổ sung trong migration store: tool_config, scheduled_messages và auto_reply_seen; giữ nguyên messages/coverage. DB đã có namespace từng tài khoản; job còn lưu owner để kiểm tra ở claim/edit/finalize. Index due, recent và attempts phục vụ lịch gửi và giới hạn auto-reply. Mọi SQLite/config/picker đi qua worker, kết quả UI có owner và query để bỏ kết quả cũ. Không có backend ngoài app.

Tìm SQLite dùng hàm fold tiếng Việt, bound query 512 byte và 50 kết quả, lọc hội thoại ẩn; không chỉ tìm resident messages. Mở kết quả đọc tối đa 81 tin ngữ cảnh theo đúng thread/message, không fetch mạng hay hydrate toàn bộ lịch sử.

Import chỉ SQLite/schema-compatible và JSON danh sách threads hoặc object chứa threads; không giải mã backup Zalo PC, không tuyên bố hỗ trợ JSONL. Nguồn SQLite mở READ_ONLY/query_only, không chạy migration nguồn; đọc từng row với guard kích thước. JSON tối đa 32 MiB. Cần xác nhận sở hữu, kiểm tra owner nếu JSON khai báo, không cho nguồn trùng DB đích. Thư mục nhận DB và hai JSON lịch sử chuẩn, bỏ qua session/settings. Một transaction bao lượt import, kiểm tra logout trong quá trình, giữ metadata/recall/flags và dùng đối soát ID chính xác; nguồn pending không thể nhập quyền gửi/retry. Không nhập scheduler/rules từ lịch sử.

Theme và sticker JSON lưu trong cấu hình từng account, validate trước khi thay; tối đa 64 pack, 500 ID/pack, màu #RRGGBB. Nạp/xuất qua picker, ghi export nguyên tử không sửa ACL thư mục người dùng. Sticker là danh sách ID Zalo, không phải tạo sticker mới từ ảnh. UI load cache cấu hình trong RAM, không đọc JSON mỗi frame.

Scheduler giữ client ID và payload, tối đa 1000 pending, quản lý 100 mục. Hẹn tương đối hoặc giờ máy YYYY-MM-DD HH:MM. App phải chạy/đăng nhập để gửi; pending quá giờ được gửi khi mở lại. Chỉ sửa body của first attempt chưa gửi; retry uncertain giữ payload. Claim một tin trong transaction, một job tự động đang chạy. Đối soát ack trong lịch sử trước khi claim/retry. Crash từ sending phục hồi uncertain, không auto-replay; lỗi gửi dừng lịch pending tới khi người dùng tiếp tục. Resume không tự requeue uncertain. Tất cả gửi dùng outgoing pipeline bền vững của fork.

Auto-reply mặc định tắt, tối đa 64 rule có exact/contains/prefix, phạm vi all/direct/group. Chỉ tin Live có timestamp sau lúc cấu hình được bật, bỏ outgoing/system/non-unread; History import không kích hoạt. Seen ID và enqueue chung transaction, tối đa năm enqueue/phút và năm attempt tự động/phút. Auto job quá 60 giây hết hạn, không trả lời trễ hàng loạt. Đổi cấu hình hủy auto jobs còn pending; không tự gửi lại sau lỗi.

## Verification / rollback

Test migration 2/4 → 6, nguồn DB không bị sửa, rollback khi dữ liệu sau không hợp lệ, Unicode/wildcard/hidden search, context exact ID, owner isolation, claim/recover/ack/retry/edit, opt-in/replay/history/rate cap. Headless/native smoke có năm tab tiện ích. Không dùng account thật hay gửi tin trong test.

Bản schema 5 trở xuống sẽ từ chối DB 6. Không hạ user_version hay xóa DB. Rollback cần binary hiểu schema 6 hoặc bản sao lưu riêng trước nâng cấp. Không có destructive migration; có thể vô hiệu hóa tiện ích trong mã/schema-6-compatible mà vẫn giữ lịch sử.
