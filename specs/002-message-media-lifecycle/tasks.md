# Công việc: Vòng đời tin nhắn, media và reaction

Checkbox mô tả công việc cụ thể, không thay trạng thái nghiệm thu toàn tính năng.
Ticket lifecycle theo docs/standards/issue-tracker.md; các ID dưới là task cục bộ.

- [x] [M-T001] [M-01,M-02,M-03,M-04,M-05] Ghi baseline mã/fixture và giới hạn hiện có vào `verification.md`; trạng thái: đã có mã. Việc này chỉ ghi nhận bằng chứng cũ, không tuyên bố hoàn tất tính năng.
- [ ] [M-T002] [M-02] Rà toàn bộ đường library/search/bubble/viewer/clipboard/open-cache và nguồn tin thiếu context; bổ sung kiểm chứng chặn chia sẻ nhất quán. — `src/gui/worker.rs; src/gui/modals.rs; src/gui/search_panel.rs; src/outgoing.rs`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [M-T003] [M-01,M-03] Rà cleanup cache media thông thường, download/renew đang chạy, đóng native viewer và lỗi xóa file; chỉ dọn cache app sở hữu. — `src/message_lifecycle.rs; src/gui/worker.rs; src/video_player.rs`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [M-T004] [M-04] Rà reactor lookup, clear, target chưa về, preview khi có tin mới, tên/avatar lỗi; ghi rõ danh sách thiếu thay vì suy ra người. — `src/reaction_pipeline.rs; src/gui/workspace.rs; tests/`; trạng thái: đã có mã; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.

- [ ] [M-T090] [M-01,M-02,M-03,M-04,M-05] Nghiệm thu các tình huống thiết bị trong `spec.md`, ghi build/thiết bị/kết quả vào `verification.md`; trạng thái: chưa xác nhận trên thiết bị.
- [ ] [M-T099] Đối chiếu tất cả yêu cầu với mã, test và release scope; giải quyết hoặc công khai phần chưa đạt trước release; trạng thái: một phần.

## Phụ thuộc

T001 là baseline. Các task độc lập có thể thực hiện tiếp khi được ủy quyền.
T090 cần thiết bị/tài khoản thử được phép; không đánh dấu xong bằng Cargo test.
