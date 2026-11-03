# ADR-0010: Đồng bộ điện thoại với ít yêu cầu khởi tạo

- Status: accepted
- Date: 2026-11-02
- Supersedes: lựa chọn chia 7/90 ngày và tối đa 30 partition/yêu cầu trong ADR-0007.

## Context

Maintainer yêu cầu giảm số lần khởi tạo chuyển lịch sử, không chia ba cửa sổ thời gian. Số gói trả về không phải số yêu cầu sync. Bằng chứng `re-notes/11_phone_sync.md` xác nhận mảng queries với partition/from/to và batchSize; chưa chứng minh 30 là trần bắt buộc phía Zalo. Không có bằng chứng ngưỡng nào gây khóa tài khoản.

## Decision

Một lần bấm có một lượt lấy danh sách hội thoại, rồi gom mọi khoảng thiếu vào ít yêu cầu message nhất. Với payload vừa ngân sách 64 KiB và chưa có giới hạn đã học, mục tiêu là hai yêu cầu khởi tạo tổng cộng, không phải một yêu cầu cho mỗi gói. 64 KiB là giới hạn cục bộ bảo thủ, không tuyên bố là giới hạn Zalo. Giữ thứ tự hội thoại gần đây, wire/cipher/chấp thuận và batchSize=2000.

Lần đầu yêu cầu từ 0; chỉ chuyển sang delta từ cuối coverage khi đã có coverage từ 0. Nếu còn lỗ ở quá khứ, yêu cầu lại một khoảng bao phủ lỗ thay vì suy rằng vùng đó đã đầy đủ. Chế độ gần đây vẫn tùy chọn, dùng 7 ngày theo thời điểm bắt đầu. Khoảng nối có overlap 2 phút, giữ quy tắc idempotency và dữ liệu chưa nhận diện.

Chỉ chia sau phản hồi từ chối rõ số partition/conversation/payload, khi chưa có xác nhận hoặc batch nào. Dùng mức tối đa được nêu hoặc giảm đôi, tối đa bốn lần điều chỉnh mỗi thao tác. Parser không đoán mã số Zalo: chỉ chấp nhận các reason máy đọc được được liệt kê trong policy; chưa có capture lỗi giới hạn thật. Mọi lý do/mã không nhận diện đều dừng. Auth/protocol/429 không cho phép chia. Giới hạn được chấp nhận lưu riêng tài khoản, giới hạn payload kiểm tra trước khi gửi; các yêu cầu tách cách nhau tối thiểu hai giây.

429/rate limit dừng và lưu cooldown theo phản hồi (5–86400 giây, mặc định 300 khi thiếu). Click tiếp trong cooldown không phát yêu cầu sync. Timeout, mất kết nối, hủy và lỗi không rõ kết quả không tự tạo lượt mới. Không gửi lại yêu cầu vào epoch socket mới.

Chờ `isLast` đúng syncId trước khi coi các mô tả gói là đầy đủ; việc đã thấy mọi partition không đủ chứng minh các partition đó không còn gói tiếp. Nhập từng gói trước; chỉ ghi coverage sau khi tất cả gói của yêu cầu giải mã/nhập thành công. Một khoảng rời coverage chờ trong RAM tới khi nối được; crash có thể khiến tải lại vùng rời, không bao giờ đánh dấu lỗ là đã hoàn tất.

## Verification / rollback

Fixture tổng hợp kiểm tra 300 partition gom một message request, ngân sách JSON, từ chối rõ/không rõ/đã accepted, cursor/delta và hoàn tất theo isLast. Cần điện thoại thật để xác minh chấp nhận số partition lớn và hình dạng lỗi giới hạn. Không tự chạy transfer thật trong CI. Không có thay đổi mật mã hoặc schema coverage. Rollback thuật toán được bằng mã tương thích schema hiện tại; không xóa tin hay hạ user_version.
