# Công việc: Quyền và quản trị nhóm

Checkbox mô tả công việc cụ thể, không thay trạng thái nghiệm thu toàn tính năng.
Ticket lifecycle theo docs/standards/issue-tracker.md; các ID dưới là task cục bộ.

- [x] [G-T001] [G-01,G-02,G-03,G-04,G-05] Ghi baseline mã/fixture và giới hạn hiện có vào `verification.md`; trạng thái: đã có mã. Việc này chỉ ghi nhận bằng chứng cũ, không tuyên bố hoàn tất tính năng.
- [ ] [G-T002] [G-01,G-03] Đối chiếu control nhóm/community thực và hoàn thiện invalidate/refresh quyền tức thời; chưa có hợp đồng thì ghi blocker, không đoán frame. — `src/group_state.rs; src/gui/worker.rs; re-notes/31-v007-contracts.md`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [G-T003] [G-01,G-02,G-03] Bổ sung fixture role/community, revoke/leave, account switch, lỗi/mất ACK và kết quả metadata đến muộn. — `src/groups.rs; src/gui.rs; tests/`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [G-T004] [G-04] Rà liên kết poll/note với nhóm nguồn, quyền mutation và refresh/phân trang sau thay đổi, bổ sung fixture theo bằng chứng. — `src/group_content.rs; src/gui/content_panel.rs`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [G-T005] [G-05] Xác nhận normalize nhóm chung, nhóm đã rời và kết quả lỗi/thiếu metadata không gây trùng hoặc mở sai nhóm. — `src/groups.rs; src/gui/group_panel.rs`; trạng thái: đã có mã; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.

- [ ] [G-T090] [G-01,G-02,G-03,G-04,G-05] Nghiệm thu các tình huống thiết bị trong `spec.md`, ghi build/thiết bị/kết quả vào `verification.md`; trạng thái: chưa xác nhận trên thiết bị.
- [ ] [G-T099] Đối chiếu tất cả yêu cầu với mã, test và release scope; giải quyết hoặc công khai phần chưa đạt trước release; trạng thái: một phần.

## Phụ thuộc

T001 là baseline. Các task độc lập có thể thực hiện tiếp khi được ủy quyền.
T090 cần thiết bị/tài khoản thử được phép; không đánh dấu xong bằng Cargo test.

## Cập nhật mã v0.008 — 06/11/2026

G-T002/G-T003: control WS/LP invalidate, epoch/runtime generation, volatile ACL,
revision chống kết quả cũ, creator numeric/string và fixture đã có. Chưa đóng phần
control thiết bị thực. G-T004: poll/note bound board/list theo nhóm và kind, creator
server, phân trang có giới hạn đã có mã/test; native vẫn ở G-T090. G-T005 giữ mã
normalize nhóm chung, cần đối soát nhóm đã rời trên tài khoản thật. Xem spec008 và
re-notes/35; không dùng kết quả Linux để đóng G-T090/G-T099.
