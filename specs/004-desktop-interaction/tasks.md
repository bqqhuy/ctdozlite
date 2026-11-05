# Công việc: Hiển thị, flicker và điều hướng hội thoại

Checkbox mô tả công việc cụ thể, không thay trạng thái nghiệm thu toàn tính năng.
Ticket lifecycle theo docs/standards/issue-tracker.md; các ID dưới là task cục bộ.

- [x] [U-T001] [U-01,U-02,U-03,U-04] Ghi baseline mã/fixture và giới hạn hiện có vào `verification.md`; trạng thái: đã có mã. Việc này chỉ ghi nhận bằng chứng cũ, không tuyên bố hoàn tất tính năng.
- [ ] [U-T002] [U-01] Rà các bề mặt còn dùng màu cố định và lập checklist ánh sáng/tối cho hai vùng ảnh lỗi đã báo. — `src/gui/theme.rs; src/gui/workspace.rs; src/gui/modals.rs`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [U-T003] [U-02] Kiểm tra fallback font emoji/ZWJ trong danh bạ và tên nhóm trên native Windows, ghi screenshot không lộ dữ liệu cá nhân. — `src/gui/theme.rs; src/gui/contacts.rs; src/gui/workspace.rs`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [U-T004] [U-03] Đo render/lịch sử với nhóm lớn và DB có sẵn; ghi dataset và số request trước/sau theo chuẩn benchmark. — `src/group_metadata.rs; src/history_open.rs; src/gui/workspace.rs`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [U-T005] [U-04] Rà mute/unread ở account switch, đổi từ điện thoại, refresh danh sách và bộ lọc kết hợp. — `src/gui.rs; src/gui/workspace.rs; tests/`; trạng thái: đã có mã; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.

- [ ] [U-T090] [U-01,U-02,U-03,U-04] Nghiệm thu các tình huống thiết bị trong `spec.md`, ghi build/thiết bị/kết quả vào `verification.md`; trạng thái: chưa xác nhận trên thiết bị.
- [ ] [U-T099] Đối chiếu tất cả yêu cầu với mã, test và release scope; giải quyết hoặc công khai phần chưa đạt trước release; trạng thái: một phần.

## Phụ thuộc

T001 là baseline. Các task độc lập có thể thực hiện tiếp khi được ủy quyền.
T090 cần thiết bị/tài khoản thử được phép; không đánh dấu xong bằng Cargo test.
