# Công việc: Khôi phục lịch sử V1 và backup Desktop V2

Checkbox mô tả công việc cụ thể, không thay trạng thái nghiệm thu toàn tính năng.
Ticket lifecycle theo docs/standards/issue-tracker.md; các ID dưới là task cục bộ.

- [x] [H-T001] [H-01,H-02,H-03,H-04,H-05] Ghi baseline mã/fixture và giới hạn hiện có vào `verification.md`; trạng thái: đã có mã. Việc này chỉ ghi nhận bằng chứng cũ, không tuyên bố hoàn tất tính năng.
- [ ] [H-T002] [H-01,H-02,H-03] Thêm fixture pipeline V1 hoàn chỉnh (RSA/archive/mapping/import), gói sai phiên, hủy giữa các giai đoạn và account switch. — `src/phone_sync/legacy.rs; tests/fixtures/; tests/`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [H-T003] [H-03] Đối chiếu BinNet/media V1 bằng nguồn đáng tin; chỉ decode phần có bằng chứng và fixture, giữ raw phần còn lại. — `src/phone_sync/legacy.rs; re-notes/31-v007-contracts.md`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [ ] [H-T004] [H-04,H-05] Rà thay file giữa dry-run/import, lỗi/cancel giữa lô, cleanup asset chưa commit và shared asset; thêm fixture tương ứng. — `src/desktop_backup.rs; src/desktop_backup_format.rs; tests/desktop_backup_v2.rs`; trạng thái: một phần; bằng chứng cần: diff, fixture/kiểm tra đúng triệu chứng và giới hạn còn lại.
- [x] [H-T005] [H-01,H-02] Rà phụ thuộc RSA prerelease và chạy cargo audit trước release; ghi kết quả, không gọi audit sạch khi chưa chạy. — `Cargo.toml; Cargo.lock; docs/adr/0033-v1-rsa-backend-release-audit.md`; trạng thái: qua kiểm thử; RSA advisory đã loại bỏ, AWS-LC/OpenSSL fixture đạt, audit đầy đủ 0 vulnerabilities; còn warning được ghi tại release verification.

- [ ] [H-T090] [H-01,H-02,H-03,H-04,H-05] Nghiệm thu các tình huống thiết bị trong `spec.md`, ghi build/thiết bị/kết quả vào `verification.md`; trạng thái: chưa xác nhận trên thiết bị.
- [ ] [H-T099] Đối chiếu tất cả yêu cầu với mã, test và release scope; giải quyết hoặc công khai phần chưa đạt trước release; trạng thái: một phần.

## Phụ thuộc

T001 là baseline. Các task độc lập có thể thực hiện tiếp khi được ủy quyền.
T090 cần thiết bị/tài khoản thử được phép; không đánh dấu xong bằng Cargo test.
