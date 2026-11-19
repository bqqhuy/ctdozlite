# Audit và phản biện chéo — 2026-11-19

Baseline: main `50d6abe`. Người dùng cho phép audit, sửa, commit, merge vào
`main`, push và chạy GitHub Actions để tạo artifact test. Không tạo release/tag.

## Báo cáo

- [Code, logic và coverage](code-audit.md)
- [Kiểm kê và parity Zalo PC](pc-parity.md)

Hai auditor độc lập phản biện chéo, một agent sửa triển khai các phần được giao.
Các vòng review tìm thêm lỗi trong chính diff sửa; không dùng đồng thuận thay cho
compiler, regression hoặc nghiệm thu thiết bị.

## Kết quả sửa trong workspace

- Link send dừng trước network khi không đọc được policy E2EE.
- Fixture integration có storage riêng từ lúc khởi động subprocess; test logout
  không dùng phiên thật.
- Kết quả history kiểm tra runtime ngay tại GUI receiver; giữ thông báo lỗi tải
  server khi vẫn có dữ liệu local.
- To-do/poll mang runtime/thread và xóa cache khi chuyển nhóm.
- Parser calltime/misscall theo params JSON, direction/duration precedence,
  reason theo chủ thể và biểu tượng video.

Auditor code và parity đã chấp nhận các diff trong phạm vi trên. Không có finding
dead code được chứng minh đủ để xóa rộng. Không khẳng định toàn dự án không còn lỗi.

Lượt tiếp tục bổ sung các sửa có regression: WSS-only trước khi gắn cookie
session; rollback backup qua transaction khi WAL còn hoạt động; ngân sách cache
video đồng bộ bằng mutex; khóa privacy cho session Windows Hello cũ; preview tài
liệu gắn owner/runtime và được dọn ở relock/logout/profile; giới hạn dòng HTTP
CONNECT trước allocation; media store đối chiếu E2 bundle với service động,
cursor `last_id`, nhánh `send2me`, schema và group prefix; fail-closed các action
thiết bị/2FA/todo/catalog chưa có caller/contract được xác thực. Parity server
chỉ đạt E2/source-level, chưa có live capture.

## Kiểm chứng

- `cargo test --locked --test audit_regressions -j 1`: 29 pass.
- Test GUI history runtime: pass.
- Nhóm test `native_` trước các sửa cuối: 13 pass, 1 ignored (MFT H.264 cần thiết bị).
- Full suite `cargo test --locked -j 1 -- --test-threads=1` với `CTDOZ_DATA_DIR`
  tạm: 982 unit pass, 10 ignored; toàn bộ integration và doc-test pass.
  Thất bại ban đầu do resolver không nhận diện URL canonical khi host candidates
  được thay bởi fixture; đã sửa canonical/discovered matching và các test session
  liên quan pass.
- `cargo check --locked`, `cargo clippy --all-targets --locked -- -D warnings`
  và `git diff --check`: pass.
- Regression sau lượt sửa tiếp tục: media store 8/8; video cache 10/10; proxy
  CONNECT 1/1; fail-closed device/2FA/todo/catalog 4/4; auth guard 14/14;
  remembered-host 9/9; stalled-body deadline 1/1.

## Giới hạn còn lại

ASAR được kiểm kê và phân tích tĩnh có mục tiêu; chưa dịch ngược toàn bộ native
binary hoặc review từng dòng của 159 Rust source. Static E2 không chứng minh
contract live, entitlement, AEC/camera/codec hoặc parity pixel. Các chuỗi gọi thật,
nghe trên thiết bị khác và ARM64 còn cần nghiệm thu. Không có API lịch sử cuộc gọi
độc lập được xác nhận từ phạm vi phân tích; recommended call rows đi qua message sync.
Graphify toàn repository đã lập đồ thị cấu trúc (576 file phát hiện, 224 file code
được parse; không có semantic/LLM extraction). Đây không thay thế source review:
ledger vẫn có các file chỉ inventory hoặc đọc có mục tiêu, chưa được chứng nhận
toàn bộ.
