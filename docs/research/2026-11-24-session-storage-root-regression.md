# Mất phiên khi chạy build có chữ `test` trong tên

Ngày: 24/11/2026 (Asia/Saigon).

## Nguyên nhân đã tái hiện

Commit `fdca94201dd7fe9d3da1408fbebd7e10a49658c8` ngày 17/11 thêm guard
tránh kiểm thử truy cập dữ liệu thật. Guard tìm chuỗi `test` trong mọi phần tử
`std::env::args()`, kể cả đường dẫn EXE và giá trị `--profile`.

ZIP `dist/ZaloLite-v0.016-arm-test-0d62e22.zip` ngày 24/11 chứa EXE
`ZaloLite-v0.016-arm-test-0d62e22.exe`. Tên này luôn kích hoạt guard khi không
có `CTDOZ_DATA_DIR` và không dùng `--portable`. Thư mục dữ liệu trở thành
`%TEMP%\ctdoz_test_data_<PID>`, thay cho `%LOCALAPPDATA%\ctdozlite`.
Mỗi lần mở có PID khác nên không đọc lại phiên, cài đặt và lịch sử ở lần trước.
Không cần giả thuyết máy chủ thu hồi phiên để tái hiện lỗi này.

Quan sát chỉ đọc trên máy: nhiều thư mục `ctdoz_test_data_<PID>` ngày 24/11
có cả `session.dat` và `profiles.json`. Không sao chép, xóa hay di chuyển dữ
liệu tài khoản để sửa lỗi.

## Bản sửa

`src/storage.rs` chỉ nhận cờ chính xác `--test` trong đối số sau đường dẫn
EXE. Giữ nguyên `cfg!(test)`, các biến môi trường kiểm thử và ưu tiên
`CTDOZ_DATA_DIR`. Tên EXE, thư mục và profile có chuỗi `test` không đổi root.

Đây là sửa đường dẫn lưu trữ nội bộ. Parity with Zalo PC/Web (source:
`re-notes/01_login.md`, §6, E2): phiên đã lưu phục vụ tự đăng nhập ở lần mở
sau. Bằng chứng E2 đó không xác nhận toàn bộ chính sách hết hạn phía máy chủ;
bản sửa không đổi endpoint, params, ký số, mã hóa hay xử lý thu hồi phiên.

## Kiểm chứng

- Trước sửa: `cargo test --locked --test storage_root_selection` thất bại;
  child process chạy binary thật với tên EXE của ZIP chọn root tạm theo PID
  thay vì user-data root giả đã cô lập.
- Regression chạy binary qua `--doctor` hai lần cho mỗi tổ hợp: tên EXE có
  `test`, thư mục có `test`, tên bình thường; không profile và `--profile=latest`.
  Mỗi child xóa các biến môi trường kiểm thử kế thừa và dùng user-data root giả,
  không đọc phiên thật và không gọi Zalo.
- Sau sửa: `cargo test --locked --test storage_root_selection --test storage_lifecycle`
  đạt 2/2 test; regression root bao gồm 12 lần chạy binary trong child process.
- `cargo test --locked --lib storage::tests::`: đạt 8/8 test, gồm DPAPI,
  atomic write, account/profile isolation và chặn runtime cũ xóa phiên mới.
- `cargo check --locked`, `cargo fmt --all -- --check`, `git diff --check`: đạt.
- Kiểm tra trước commit: `cargo check --locked` và
  `cargo clippy --all-targets --locked -- -D warnings`: đạt.
  Lượt link integration test có cảnh báo LNK4199 về bốn DLL Media Foundation
  không có import; không có lỗi biên dịch.

Chưa nghiệm thu đóng/mở GUI với tài khoản thật trên binary đã sửa. Không tự
khôi phục phiên từ các thư mục PID vì có thể chứa nhiều lần đăng nhập/tài khoản
khác nhau; token cũ cũng có thể đã bị thu hồi bởi lần đăng nhập sau.
