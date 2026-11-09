# Khắc phục audit độc lập ngày 09/11/2026

Phạm vi: working tree trên `f842dfeb84fd94ff7ea79dc8d6b88dc9ad9d94bc`.
Người dùng đã cho phép commit sau kiểm chứng; không push, tag hoặc phát hành.
Các thay đổi có sẵn khi bắt đầu được tiếp tục kiểm tra và hoàn thiện, không reset
cây làm việc. Commit chỉ lưu mã, không chạy migration trên dữ liệu tài khoản thật.

## Đối chiếu phát hiện

| Mục audit | Thay đổi |
| :--- | :--- |
| 1. Lỗi đọc DB thành plaintext | API kiểm tra trả `Result`; wrapper bảo thủ không cấp phép plaintext khi có lỗi; yêu cầu E2EE tồn tại độc lập với ratchet. |
| 2. Mất prekey khi ghi phiên lỗi | Transaction RAII bao trọn xử lý prekey, dấu replay, session và trusted identity; test chèn lỗi ghi DB. |
| 3. Khóa danh tính giả | Bỏ fallback hash; từ chối verify khi thiếu khóa; migration xóa placeholder cũ; verify ràng buộc đúng mã đang hiển thị. |
| 4. Mất cảnh báo đổi danh tính | Metadata cảnh báo theo tin nhắn, bỏ cache verified cũ, kết quả theo owner; không dùng mã tự tạo để xác minh Zalo thật. |
| 5. Mất gói khởi tạo | Model local giữ prekey envelope đến khi có phản hồi đã xác thực, tiếp tục phiên có cùng base key, vẫn từ chối replay. |
| 6. Hợp đồng E2EE chưa có bằng chứng | Chặn production send/decrypt/verify; bỏ HTTP adapter phỏng đoán. Xem ADR-0035. |
| 7. Lẫn tài khoản | Chụp session trước spawn; revoke và kết thúc worker cũ; worker/mailbox/event stream mới cho profile đích; kiểm tra UID khớp file phiên. |
| 8. Payload ngoài text bỏ E2EE | Kiểm tra chính sách trước chọn loại payload; chặn non-text và không downgrade. |
| 9. Khóa app bỏ sự kiện | Tiếp tục cập nhật business state trong khi input/render vẫn bị khóa; regression test. |
| 10. Broadcast lag dừng progress | Bỏ qua `Lagged`, chỉ dừng khi channel đóng; regression test làm tràn channel. |
| 11. Response lỗi thành rỗng | Kiểm tra envelope và dữ liệu bắt buộc cho 2FA/catalog; giữ mã lỗi ngoài của bundle. |
| 12. Tin nhắn nhanh chỉ trong RAM | Lưu trong preferences của DB tài khoản qua worker; kết quả save/load cập nhật UI; bỏ dữ liệu mẫu khỏi default. |
| 13. DNS lọt IP nội bộ | Kiểm tra IP literal và toàn bộ kết quả DNS, nối đúng các địa chỉ đã kiểm tra; không scrape qua proxy có DNS từ xa không kiểm soát được. |
| 14. I/O trên UI | Lưu settings và nạp registry qua worker, render từ cache profile. |
| 15. Vòng đời prekey thiếu nối | Pool local tự bổ sung khi cần và giữ high-water mark định danh; upload server vẫn bị chặn vì thiếu hợp đồng. |
| Hiệu năng | Checkpoint prefix gọn, thêm chunk tuần tự O(1), tìm gap O(log n), prune skipped keys theo batch; cache thứ tự sidebar và virtualize hàng. |
| Code dư | Bỏ dialog orphan, helper trùng/không có caller và adapter E2EE phỏng đoán; gom endpoint vào bảng chung; sửa comment sai và format Rust. |
| Phát hiện khi chạy mở rộng | Test H.264 native từng không trả frame; cấu hình low-latency MFT theo tài liệu Microsoft. Kết quả kiểm chứng lại được ghi bên dưới. |

## Bằng chứng protocol

**Unknown — AMBIGUOUS:** định dạng E2EE chính thức và schema prekey chưa được xác
minh trong `re-notes/36`, mục 8. Route index E2 không chứng minh schema. Vì vậy
thay đổi không phát request mới hoặc tự khẳng định parity E2EE. Các test crypto
dùng hai peer Rust và khóa tổng hợp; không chứng nhận Zalo interoperability.

Endpoint và header đã có chỉ được gom về `endpoints.rs`, giữ nguyên giá trị và
tham số. Không tự retry mutation hoặc thay cipher/signkey của request đang có.

## Kiểm tra

Máy kiểm tra: Windows x64, Intel Core i5-12400F, bản debug. Mọi phiên/dữ liệu thử
được đặt trong thư mục tạm riêng qua `CTDOZ_DATA_DIR`; không sử dụng tài khoản thật.

| Kiểm tra | Kết quả thực tế |
| :--- | :--- |
| `cargo test --locked --no-fail-fast -j 2` sau sửa H.264 | **803 passed, 0 failed, 11 ignored** |
| Chạy riêng toàn bộ ignored, một thread | 10 passed, 1 failed ban đầu: `native_h264_roundtrip` không trả frame |
| Chạy lại `native_h264_roundtrip` sau cấu hình low-latency | **1 passed**, không có lỗi; đây là Windows x64, không phải ARM64 |
| Regression chuyển tiếp media từng flaky | 5 lượt độc lập liên tiếp đạt sau khi cô lập tài khoản fixture và nới thời gian chờ nhận request |
| Codec đồng bộ chính thức | Passed với WASM công khai đã kiểm tra SHA-256 theo pin trong repo |
| `powershell -NoProfile -File scripts/audit-prepush.ps1` | Exit 0; cargo check và Clippy `-D warnings` đạt |
| `cargo check --locked --all-targets --target aarch64-pc-windows-msvc -j 2` | Đạt; dùng clang có sẵn trong `C:\Program Files\LLVM\bin` |
| UI smoke 1020×700 dark và 480×360 light | Exit 0, có PNG; đã kiểm tra hình thực tế, sidebar và khung soạn thảo hiển thị |
| `cargo audit --json` | 0 vulnerabilities; 5 informational warnings, không gọi là audit hoàn toàn sạch |

Script prepush có một cảnh báo heuristic về nút gửi tệp khi composer còn mention.
Đã đọc nhánh đầy đủ: đây là validation từ chối đầu vào không hỗ trợ; nhánh hợp lệ
vẫn gửi `WorkerCmd::PickAndSendAttachment`, không phải nút giả. Linker x64 test
có LNK4199 cho delay-load DLL không được import; không phải lỗi compiler.

Lượt build nhiều target song song đầu tiên hết RAM. Các lượt kết luận chạy lại
với `-j 2`; không tính lượt bị hủy/lỗi tài nguyên thành kết quả đạt. Test trace cũ
đọc `session.json` và gọi tài khoản thật đã được xóa trước khi chạy ignored.

### Đo checkpoint mới

Test `sequential_upload_checkpoints`: 4.096 chunk, 7 vòng, đo thuật toán theo dõi
và serialize checkpoint; không đo tốc độ mạng hoặc I/O SQLite.

| Thuật toán | min ms | avg ms | p50 ms | p95 ms | p99 ms |
| :--- | ---: | ---: | ---: | ---: | ---: |
| Cũ: contains/sort + serialize danh sách đầy đủ | 1133.561 | 1228.493 | 1234.524 | 1353.759 | 1353.759 |
| Mới: append/binary search + prefix gọn | 1.878 | 2.008 | 1.905 | 2.369 | 2.369 |

Thời gian CPU trung bình giảm khoảng 612 lần trong dữ liệu tổng hợp này. Không
suy ra rằng gửi tệp thực tế nhanh hơn cùng tỷ lệ. Các benchmark khác đã chạy,
bao gồm tìm kiếm 632.000 hàng và nhập 5.000 tin. Một nhãn môi trường trong benchmark
cursor cũ được hardcode là Linux; máy thực thi lần này là Windows như ghi ở trên.

### Cảnh báo dependency

- Unmaintained: `audiopus_sys 0.2.2`, `paste 1.0.15`, `proc-macro-error 1.0.4`,
  `ttf-parser 0.25.1`.
- Unsound informational advisory: `glib 0.18.5`. `cargo tree --target
  aarch64-pc-windows-msvc -i glib -e normal` xác nhận package này không nằm trong
  dependency tree Windows ARM64.
- Không thay dependency hoặc suppress advisory để tạo kết quả sạch giả.

Không đánh dấu các task nghiệm thu thiết bị ARM64 hoặc Zalo thật hoàn tất từ
unit test. Kho crypto nâng schema lên 6; binary cũ có guard schema sẽ từ chối
mở kho mới. Không hạ `user_version` thủ công để rollback. Nếu cần rollback bản
phát hành sau này, dùng bản vá tương thích schema hoặc bản sao lưu nhất quán đã
được tạo trước khi chạy bản nâng cấp; không xóa khóa/lịch sử để né guard.

## Nghiệm thu còn cần thiết bị

1. Đổi hai profile thật trong khi tải media/gửi tin/đồng bộ; xác nhận không có dữ
   liệu hoặc sự kiện tài khoản cũ xuất hiện ở tài khoản mới.
2. Windows Hello: khóa/mở khi upload hoàn tất, kiểm tra state không mất và nội
   dung vẫn không hiển thị khi khóa.
3. Sidebar nhiều hội thoại: cuộn ở DPI khác nhau và kiểm tra chọn đúng thread,
   menu ngữ cảnh, pin, unread và lọc.
4. E2EE: chỉ mở lại sau khi có contract chính thức, fixture và đối chiếu native.
