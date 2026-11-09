# Khắc phục audit độc lập trên 164af11

Giữ E2EE thử nghiệm và log đã che dữ liệu. Không tạo release/tag.

| Mục audit | Thay đổi |
| :--- | :--- |
| 1 — TUI/API thấp bỏ qua E2EE | Guard trước quote/sticker/attachment/contact/forward; hồi quy gọi trực tiếp API thấp |
| 2 — Envelope giả đổi chính sách | Hình dạng body không tự tạo yêu cầu E2EE; chỉ metadata rõ ràng/local opt-in |
| 3 — Tin nhóm đổi policy DM | Parser nhận ngữ cảnh nhóm trước khi chọn đường crypto; hồi quy groupMsgs |
| 4 — Nhận trùng/lỗi lưu lịch sử | Receipt có ID/hash, plaintext bảo vệ DPAPI cùng transaction ratchet; giữ plaintext cũ khi gặp placeholder; purge khi xóa/thu hồi |
| 5 — Tràn ID bundle | Chuyển kiểu có kiểm tra; bắt buộc ID, không bỏ qua key/signature sai |
| 6 — Device list sai schema thành rỗng | Result và kiểm tra envelope/list/identity; rỗng hợp lệ khác thiếu dữ liệu |
| 7 — Thông báo khi khóa | Lock theo snapshot, thu hồi preview/activation và ngăn publisher cũ đổi owner |
| 8 — Registry đa tài khoản không có dữ liệu | Đăng nhập thật lưu phiên/registry; thêm tài khoản, chọn và khôi phục sau restart; chặn stale writer |
| 9 — Hello chặn GUI | Bỏ join trong constructor, poll kết quả nền không chặn, deadline chờ ở GUI |
| 10 — LAN chưa có hợp đồng | Bỏ scanner/probe tự định nghĩa, thông báo chưa khả dụng; mở lại các task C03 |
| 11 — Host hardcode | Dùng endpoints::FRIEND, giữ nguyên giá trị/giao thức |

Tối ưu: dùng map định vị hội thoại thay position cho từng message; giữ nguyên
ngữ nghĩa vị trí đầu tiên khi đầu vào có ID trùng. Phần lookup đổi từ O(M*T)
sang O(M+T). Tái sử dụng connection crypto theo worker thread/account.

Dọn: bỏ model LAN không có luồng truyền thực tế, event/command/UI scanner,
helper đường dẫn profile không dùng, hover_profile và wrapper Hello không có caller.
Không bỏ renderer account switcher đang được dùng.

Lỗi SQLite phát hiện trong hồi quy đồng thời được sửa bằng IMMEDIATE transactions,
busy timeout trước WAL và kiểm tra lại version sau khi giành quyền migration.
Nâng SQLite đi kèm để xử lý advisory của bản bundled cũ.

## Kiểm chứng

Test mới nằm ở `tests/audit_remediation_e2ee.rs`, dùng tài khoản giả, DB tạm và
child process cô lập để kiểm tra log không ghi được/drain khi thoát. Không dùng
session thật hay gửi tin tới Zalo. Các kết quả cuối và GitHub build được bổ sung
sau khi lệnh tương ứng hoàn tất; không suy nghiệm thu ARM64/Zalo từ check x64.

Crypto DB nâng lên schema 8. Xem ADR-0037 và hướng dẫn E2EE_TRIAL trước khi thử
trên dữ liệu thật; giữ bản sao dữ liệu khi app đã đóng.

### Kết quả local

- Mặc định: **812 passed, 0 failed, 12 ignored**.
- Có feature E2EE: **815 passed, 0 failed, 12 ignored**.
- Check toàn bộ target ARM64 có feature: đạt.
- `cargo audit`: **0 vulnerability** sau khi nâng bundled SQLite; 5 informational
  warnings vẫn còn (audiopus_sys, paste, proc-macro-error, ttf-parser và glib).
- PowerShell build script và YAML workflow parse thành công.
- Lượt đầu bắt được thiếu token ở fixture profile và tranh chấp WAL snapshot;
  fixture token giả và giao dịch IMMEDIATE đã được sửa, test chạy lại đạt.
- Cảnh báo linker DELAYLOAD không có import là cảnh báo hiện hữu; không suy thành
  lỗi compiler. Kiểm tra compiler/Clippy nghiêm ngặt được chạy riêng trước commit.

Benchmark trước sửa dùng 1.000 tin, lần lượt 1.000/5.000/10.000 hội thoại:
25,139 / 55,329 / 107,583 ms trong debug x64. Đây là số local CPU, không phải
throughput mạng hoặc số release ARM64. Benchmark sau sửa xuất min/avg/p50/p95/p99
với bảy vòng chạy bằng test ignored `queue_merge_lookup_benchmark`.

- Audit pre-push, Clippy all-target/all-feature nghiêm ngặt, fmt và diff check: đạt.
