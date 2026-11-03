# ADR-0005: Đồng bộ lịch sử có xác nhận trên điện thoại

- Trạng thái: triển khai để kiểm thử; cần xác nhận thực tế với tài khoản của chủ máy.
- Ngày: 2026-11-02.

## Quyết định

Bổ sung thao tác **Đồng bộ từ điện thoại** trong Settings → Đồng bộ. Không tự gửi yêu cầu xác nhận khi đăng nhập. Dùng chính WebSocket đang nhận tin trực tiếp, gửi yêu cầu 590 và thu hồi phiên bằng 591. Trạng thái, chờ xác nhận, tiến độ theo đợt/gói và thao tác dừng xuất hiện tại cửa sổ chính. Không tạo phần trăm khi không biết tổng số tin.

Lượt conversation lấy các partition; lượt message yêu cầu tối đa 30 partition/đợt, tuần tự để giới hạn áp lực lên điện thoại. Khóa identity giữ trong một lần chạy, khóa ephemeral đổi mỗi đợt. Đợi tối đa 180 giây cho lượt đầu và 600 giây/đợt tin; hủy hoặc mất kết nối dừng lượt, không tự tạo lại lời nhắc. Chạy lại có thể yêu cầu xác nhận lại; SQLite chống nhập trùng theo ID, không suy đoán theo nội dung.

## Mật mã và nguồn bằng chứng

Xem [ghi chú giao thức](../../re-notes/11_phone_sync.md). ABI và protobuf được đối chiếu với worker công khai của Zalo Web. Tải module mật mã chính thức được ghim SHA-256, chạy bằng interpreter `wasmi` trong tiến trình Rust ARM64. Không chạy JavaScript tải về, không cần Node/Electron/Chromium, không cấp WASI, mạng, tệp hay thực thi tiến trình cho module. Không đóng gói/phân phối module Zalo trong artifact; lần đầu cần mạng để tải bản có hash đúng. Nếu upstream thay/xóa asset, báo lỗi, không tự tin cậy module mới.

Client tải asset/tệp ký URL riêng không gắn cookie phiên. Kiểm tra HTTPS, host, cổng và từng redirect; URL ký không được ghi vào database/lỗi. Module giới hạn 64 MiB linear memory, heap host 64 MiB, 4096 object và instruction fuel mỗi hàm. Chunk plaintext tối đa 16 MiB; response và tổng plaintext mỗi gói tối đa 64 MiB. Giải mã/giải nén/SQLite chạy ngoài UI và ngoài socket read loop. Khóa và buffer mật mã tạm dùng zeroize; không lưu khóa riêng.

## Lưu trữ và định danh

Thêm schema **4**, bảng `phone_unmapped(partition_id,msg_id,raw_json)` chỉ giữ lịch sử chưa ánh xạ được. ID partition opaque không được biến thành recipient gửi tin. Bạn bè/nhóm dùng `globalId` từ metadata mới; người dùng ngoài danh bạ dùng profile `gid/decrypt`. Lỗi nhận diện được báo và dừng, giữ các phần đã lưu. Nhóm đã rời hoặc chưa nhận diện được ở bảng riêng, chưa có UI khôi phục/duyệt bảng này.

Mỗi partition ghi trong transaction, giữ tombstone thu hồi và cờ cục bộ. ID 64 bit giữ dưới dạng chuỗi; `globalId=0` không phải ID hợp lệ, dùng client ID có namespace, đối chiếu ID client đã lưu nếu có. Dữ liệu lịch sử vào GUI mang nguồn History, không tạo thông báo/unread như tin trực tiếp. GUI nhận tối đa 20 tin mới nhất mỗi partition; toàn bộ lịch sử nằm trên đĩa theo tài khoản. Metadata gốc được giữ cho loại tin chưa render đầy đủ. Đồng bộ mang tham chiếu tệp, không đồng nghĩa tải toàn bộ media.

## Kiểm chứng và rollback

Fixture mật mã là dữ liệu tổng hợp ngoại tuyến, không có tài khoản/khóa thực. Kiểm tra known-answer metadata/message, roundtrip, sửa ciphertext bị từ chối, parsing protobuf, chống bomb, ID lớn/0, chống lặp, thu hồi, phạm vi hoàn tất, epoch WebSocket và trạng thái GUI. CI tải riêng public asset để chạy test trên Windows ARM64; không đăng nhập hay chạm điện thoại thật.

Cần chủ máy kiểm tra: mở Zalo điện thoại → đăng nhập desktop → Settings → Đồng bộ từ điện thoại → xác nhận; so sánh chat cá nhân/nhóm cũ, tự gửi, ảnh/sticker/quote, tin trực tiếp trong lúc tải; thử dừng, mất mạng và chạy lại. Chưa có kết quả đầu-cuối với điện thoại thật thì không coi là đã chứng minh khôi phục toàn bộ lịch sử.

Schema 4 là nâng cấp cộng thêm. Binary schema 3 sẽ từ chối mở DB mới; không hạ `user_version` thủ công. Rollback bằng bản sửa vẫn hiểu schema 4 hoặc khôi phục backup theo đúng tài khoản. Không xóa DB để rollback.
