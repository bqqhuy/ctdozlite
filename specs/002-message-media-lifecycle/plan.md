# Kế hoạch: Vòng đời tin nhắn, media và reaction

Đặc tả: [spec.md](spec.md). Công việc: [tasks.md](tasks.md).

## Nền và thứ tự

1. Đọc AGENTS, standards liên quan và ADR-0031; kiểm tra diff hiện tại, giữ thay đổi người dùng.
2. Giữ module nền `src/message_lifecycle.rs, src/conversation_policy.rs, src/reaction_pipeline.rs, src/desktop_backup.rs, src/gui/worker.rs`; bổ sung hành vi còn thiếu theo từng task, không viết lại toàn bộ.
3. Với giao thức thiếu bằng chứng, ghi nguồn/giả thuyết trong re-notes trước khi đổi request.
4. Kiểm chứng worker/UI/DB ở biên tài khoản, hủy và lỗi; tránh blocking trên UI và transaction dài.
5. Rà diff và chạy các check thích hợp; cập nhật [verification.md](verification.md).
6. Thực hiện các tình huống thiết bị đã liệt kê khi có môi trường được phép, không suy từ fixture.

## Phụ thuộc và bảo toàn dữ liệu

Giữ schema 10/identity/delete marker đã có; mọi thay đổi thêm cần migration forward-only
và ADR khi thay quyết định kiến trúc. Tác vụ đang chạy phải gắn tài khoản/phiên;
response đến muộn không áp sang context khác. Mutation không lặp khi chưa rõ ACK.
Không sao chép chính sách từ standards sang đây.

## Giới hạn

Bộ kiểm thử cục bộ chưa chứng minh đầy đủ cleanup cache thường/native Windows; danh sách reactor chỉ gồm UID thật đã nhận.
Một task bị chặn không chặn các task độc lập. Chưa build/commit/push/release trong pilot.
