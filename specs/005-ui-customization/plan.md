# Kế hoạch chọn lọc PR #105

[Spec](spec.md) · [Tasks](tasks.md).

1. Rà nền src/gui/theme.rs, src/gui/modals.rs, src/gui/workspace.rs, src/gui/worker.rs,
   src/outgoing.rs và các test liên quan; giữ AGENTS/standards và identity/ACK của fork.
2. Ưu tiên font/scale/màu: một nguồn cấu hình, fallback an toàn, đọc file/font ngoài UI;
   live preview/reset và kiểm tra kích thước. Đổi cấu hình không gây ghi đĩa mỗi frame.
3. Rà picker/reply/forward, chỉ port phần chưa có và kiểm chứng được; không lấy lại
   fallback có thể gửi trùng, không bỏ qua source policy.
4. Test literal/tên trùng, font thiếu, cấu hình hỏng, account switch, Escape/focus;
   native smoke các kích thước/DPI/theme và nghiệm thu tương tác.
5. Ghi evidence vào verification.md. Chưa sửa mã cho tới khi người dùng chốt triển khai.

Không đổi schema hoặc giao thức theo mặc định. Nếu cần thay quyết định kiến trúc,
đọc ADR liên quan và ghi ADR mới; không sửa ADR đã accepted.
