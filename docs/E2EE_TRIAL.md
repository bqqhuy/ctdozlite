# Thử E2EE và lấy log

Hai thay đổi migration crypto và chặn E2EE mặc định đã có trong commit `38d5e9d`.
Bản thử bổ sung chế độ chủ động thử giao thức chưa xác minh; chưa có bằng chứng
nó gửi/nhận tương thích với Zalo chính thức. Chỉ dùng tài khoản/hội thoại kiểm tra.

## Build và chạy

Build trên môi trường đã có đầy đủ công cụ Windows ARM64 của dự án:

```powershell
$env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'
cargo build --release --locked --bin ctdozlite --target aarch64-pc-windows-msvc --features experimental-e2ee -j 2
& .\target\aarch64-pc-windows-msvc\release\ctdozlite.exe --gui --e2ee-experimental
```

Để tách dữ liệu thử khỏi dữ liệu đang dùng, đặt `CTDOZ_DATA_DIR` thành đường dẫn
tuyệt đối mới trước khi chạy, rồi đăng nhập trong bản thử. Không sao chép session
hay khóa vào log. Với dữ liệu cũ, đóng app và sao lưu toàn bộ thư mục dữ liệu
trước khi chạy. Crypto DB sẽ nâng lên schema 8; bản chỉ hỗ trợ schema 7 không mở
được DB mới. Không sửa số phiên bản DB để hạ cấp.

Build bình thường không có feature vẫn giữ chặn. Build có feature nhưng không
chạy với cờ thử cũng giữ chặn. Không cần `--dev`; không kết hợp `--dev`, `--dump`
hoặc `--dump-raw` với chế độ thử.

## Kiểm tra trong app

1. Xác nhận có dòng cảnh báo **E2EE THỬ NGHIỆM** trong giao diện.
2. Mở hội thoại riêng với tài khoản kiểm tra, mở bảng thông tin hội thoại.
3. Chọn **Bật thử E2EE cho hội thoại này**, chờ thông báo đã bật rồi gửi một
   tin văn bản kiểm tra. Hành động này lưu yêu cầu mã hóa: hội thoại sẽ không
   quay về gửi thường khi tắt chế độ thử hoặc thử thất bại.
4. Kiểm tra cả bên nhận bằng Zalo chính thức. Chỉ thấy app báo thành công chưa
   đủ chứng minh bên kia đọc được. Thử nhận lại tin, đóng/mở app và kiểm tra tiếp.
5. Nếu lỗi, không bấm gửi lại liên tục. Một client ID chỉ được thử một lần;
   kiểm tra hội thoại bên nhận trước khi soạn tin thử mới.

Ảnh, tệp, nhãn dán, trích dẫn và các nội dung E2EE chưa hỗ trợ vẫn bị chặn.
Chế độ thử không tự bật E2EE cho mọi hội thoại. Khi gửi lần đầu, app có thể thử
đăng ký khóa công khai lên máy chủ; nếu được chấp nhận, việc này có thể ảnh hưởng
trạng thái khóa của tài khoản. Dùng tài khoản kiểm tra cho lượt đầu.

## Gửi bằng chứng để phân tích

Sau khi thao tác xong, chờ vài giây rồi đóng app bình thường. Lấy hai file trong
thư mục dữ liệu của lần chạy đó:

- `diagnostics/sync-media.jsonl`
- `diagnostics/sync-media.previous.jsonl` nếu có

Có thể mở trực tiếp thư mục này từ giao diện: mở **Tiện ích hội thoại** → tab **Thiết bị & Bảo mật** → bấm **Mở thư mục nhật ký** hoặc **Sao chép đường dẫn**.
Đường dẫn gốc cũng được in khi khởi chạy; có thể xem bằng `--doctor` cùng các tùy chọn
profile/portable và `CTDOZ_DATA_DIR` đã dùng. File chứa cả chẩn đoán sync/media;
sự kiện E2EE có `event: "e2ee"`. Gửi kèm giờ thử, bước thao tác, kiến trúc máy,
commit build và việc bên nhận có nhận/đọc được hay không. Không gửi `session.json`,
`e2ee.db`, DB lịch sử hay thư mục dump.

Log ghi bước xử lý, thời gian, kích thước, bộ đếm gói, mã lỗi số và nhóm lỗi;
không ghi nội dung tin, khóa, ciphertext, cookie hoặc ID người dùng nguyên bản.
`contract` nghĩa là chưa bật đủ điều kiện thử; `fetch_bundle`/`parse_bundle`
liên quan lấy/đọc khóa; `upload_bundle` là đăng ký khóa; `encrypt_persist` là
mã hóa/lưu trạng thái; `send` là gửi HTTP; `acknowledgement` là kiểm tra xác nhận;
`decode`/`decrypt_persist` là đọc/giải mã gói nhận. `authentication` là MAC không
hợp lệ, `replay` là gói lặp, `database` là lỗi DB. `cancelled` có thể là tác vụ bị
hủy; thiếu sự kiện cuối cũng có thể do app bị tắt đột ngột hoặc hàng đợi mất log.
Log cố ý không chứa dữ liệu đủ để tái dựng khóa/nội dung hay toàn bộ schema server.

Build GitHub: chạy workflow **Windows ARM64 (native build)** trên `main`, chọn
`experimental_e2ee=true`. Artifact tên `ctdozlite-windows-arm64-trial`; có portable,
installer, checksum, build-info với SHA nguồn và log/ảnh smoke test. Đây là build
thử, không phải GitHub Release. Khi chạy vẫn cần `--e2ee-experimental`.

Nếu thư mục log không ghi được, chế độ thử không bật. Nếu writer lỗi giữa chừng,
app hiển thị lỗi và chặn lần gửi thử tiếp theo; khi thoát bình thường app đợi ghi
log với thời hạn cố định. Log GitHub có `build_revision` để phân biệt các bản thử.
