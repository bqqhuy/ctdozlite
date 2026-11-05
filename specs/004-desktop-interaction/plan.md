# Kế hoạch: Hiển thị, flicker và điều hướng hội thoại

Đặc tả: [spec.md](spec.md). Công việc: [tasks.md](tasks.md).

## Nền và thứ tự

1. Đọc AGENTS, standards liên quan và ADR-0031; kiểm tra diff hiện tại, giữ thay đổi người dùng.
2. Giữ module nền `src/gui/theme.rs, src/gui/workspace.rs, src/gui.rs, src/group_metadata.rs, src/history_open.rs`; bổ sung hành vi còn thiếu theo từng task, không viết lại toàn bộ.
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

Test tương phản và grapheme không thay kiểm chứng glyph thực tế hay quan sát flicker trên GPU Windows ARM64.
Một task bị chặn không chặn các task độc lập. Chưa build/commit/push/release trong pilot.
