# ADR-0006: Đối soát tin theo ID và điều hướng trích dẫn

- Trạng thái: accepted.
- Ngày: 2026-11-02.
- Nguồn: upstream `c65e82e3329b1cbe2c5342612a715ecf7ad04408` (#61), `752969644a180975eb8ee7fcb884144b8521a444` (#60).

## Quyết định

Đối soát phản hồi gửi, push/poll và lịch sử bằng ID server/client rõ ràng. Không dùng nội dung hoặc khoảng cách thời gian để chọn bản ghi tạm. Client ID được cấp một lần cho mỗi lần gửi, duy nhất trong tiến trình kể cả nhiều yêu cầu cùng mili giây; truyền xuyên suốt text/link/quote/sticker/forward và fallback hiện có. GUI nhận lại đúng client ID; TUI dùng cùng ID cho bản ghi tạm và request. Không coi client ID/giá trị 0 là ID server.

Index giữ các va chạm để từ chối chọn một bản ghi tùy ý. Hai ID server khác nhau vẫn là hai tin dù nội dung, thời gian và client ID trùng. Giữ biểu diễn chuỗi của ID lớn và khóa legacy đã lưu; không tự đổi namespace ID trong cache. Chỉ bản ghi gửi tạm `out_`/`local_` có đúng client ID duy nhất mới được đối soát với server. Phân biệt chiều gửi và sender; `0` là alias self trong tin outgoing của phiên hiện hành, không là bằng chứng cùng người giữa các tài khoản.

Khi phản hồi HTTP đến sau push, giữ timestamp server và trạng thái giao hàng đã biết. Khi push đến sau phản hồi gửi, SQLite chuyển khóa bản ghi tạm sang ID thật trong cùng transaction với việc lưu snapshot; không để bản ghi tạm xuất hiện lại sau hydrate. Dùng index client có sẵn trong schema 4. Không xóa các bản ghi cũ có định danh mơ hồ. Không thay schema, DPAPI, account namespace, recall tombstone hoặc local flags.

Bấm khối trích dẫn chỉ tra các tin **đã nạp trong hội thoại đang mở**. Đối chiếu chính xác canonical/server/client ID, không ép chuỗi thành số và không tìm bằng nội dung. Chỉ cuộn khi có đúng một kết quả; mở rộng cửa sổ render trong giới hạn dữ liệu đã nạp, cuộn tới hàng đích, highlight 3 giây. Tin chưa nạp hoặc ID trùng hiển thị thông báo; không tự tải toàn bộ lịch sử hay chuyển hội thoại. Bỏ search/star filter khi điều hướng thành công. Hủy trạng thái jump khi đổi hội thoại, tài khoản hoặc hết hạn highlight.

## Điều chỉnh cho fork

Giữ worker và event source Live/History, ghi SQLite theo tài khoản, native media/tray, tiến độ đồng bộ điện thoại và giới hạn RAM hiện có. Không đưa scheduler/auto-reply/REST API từ các vùng conflict upstream vào fork. Index cấp phát HashSet chỉ khi thực sự có va chạm; trường hợp thông thường giữ một chỉ số hàng.

## Kiểm chứng

Fixture tổng hợp kiểm tra nhiều tin giống nhau, echo/client ID, va chạm/namespace, chiều gửi/sender, alias self, receipt 0, request mã hóa qua loopback, cả thứ tự HTTP/push, rollback và hydrate SQLite. Kiểm thử egui bấm trích dẫn cả hai chiều và cuộn/highlight hàng cũ ngoài cửa sổ render ban đầu. Native CI thêm screenshot jump vào tin thứ 9 trong 200 tin, bên cạnh kiểm thử DX12/tray/phone sync trước đó. Không dùng tài khoản hoặc gửi tin thật.

Benchmark `examples/history_bench.rs` đo 20 lần hợp nhất 1000 vào 1000 tin với 500 trùng ID trên Linux x86_64 debug. Số đo được ghi ở PR, không đại diện hiệu năng Snapdragon. Index mới tốn thêm kiểm tra so với heuristic cũ để không gộp nhầm tin.

## Rollback và giới hạn

Rollback binary về bản schema-4-compatible trước đó được; không hạ `user_version` hoặc xóa database. Không có thay đổi dependency hay migration trong lần này. Không chứng minh tương thích server/điện thoại thật hoặc cải thiện gọi/camera. Trích dẫn tin ngoài dữ liệu đang nạp vẫn cần người dùng tải lịch sử trước.
