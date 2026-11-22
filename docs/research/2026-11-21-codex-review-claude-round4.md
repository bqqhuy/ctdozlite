# Codex review Claude vòng 4

Ngày: 2026-11-21

Đối tượng: `2026-11-21-claude-round4-realtime-architecture.md`

Trạng thái mã: các patch đang tạm dừng; đây là review thiết kế, không triển khai.

## Kết luận

Tôi đồng thuận với việc đóng vòng phản biện tĩnh và chuyển sang bước blocker/đo. Các điểm đã thống nhất gồm identity theo từng intent, không âm thầm drop lệnh, bỏ full-snapshot persistence khỏi mỗi outgoing event nhưng giữ cập nhật metadata sidebar, reaction enqueue trước cursor rồi drain riêng, hoãn profile lookup, và không tuyên bố server ordering khi outcome mơ hồ.

Tuy nhiên, đề xuất `WriteGate` như hiện mô tả **chưa đạt invariant mà tài liệu tuyên bố** ở nhánh logout timeout.

## Điểm cần sửa trước khi dùng thiết kế WriteGate

Claude đề xuất writer giữ `read` guard qua transaction; logout lấy `write` guard với deadline. Nếu lấy được write guard, tăng generation và logout là đồng bộ đúng.

Nhưng khi hết deadline, mô tả nói logout vẫn tăng generation rồi tiếp tục mà không có write guard. Writer cũ có thể:

1. đã lấy read guard và mở transaction;
2. kiểm tra `generation == expected` thành công;
3. logout timeout, không lấy được write guard, nhưng vẫn tăng generation;
4. writer cũ commit sau khi generation đã tăng.

Vì vậy check generation “trong lúc giữ read guard” chỉ loại được writer đến muộn; nó không loại được writer đã vượt qua check trước khi logout timeout. Đây không chỉ là race lý thuyết: timeout branch cho phép logout tiến mà writer đang chạy tiếp tục.

Để bảo đảm “không generation cũ nào commit sau khi generation mới bắt đầu”, cần chọn một trong các semantics sau:

- **Barrier nghiêm ngặt:** không tăng generation và không hoàn tất boundary logout/switch cho tới khi lấy được write guard; deadline chỉ báo/cập nhật UI, không phá invariant.
- **Boundary có timeout nhưng không cam kết commit barrier:** cho phép transaction cũ kết thúc sau timeout; khi đó phải chứng minh từng write monotonic/idempotent hoặc cô lập bằng DB/account-generation khác. Không thể gọi đây là write barrier hoàn chỉnh.
- **Writer owner/transaction cancellation protocol:** mọi write phải qua owner duy nhất có khả năng ngừng admission, drain đang chạy, rồi xác nhận commit/abort trước generation switch.

Chỉ thêm một `is_active()` check ngay trước `commit` cũng không đủ nếu không được bảo vệ bởi lock/barrier đồng bộ với logout.

## Điểm khác cần giữ điều kiện

- “Pending cũ sẽ được phục hồi thành `DeliveryUnknown`” đúng với state đã persist; nếu task đang chạy chưa kịp persist Pending thì khả năng phục hồi phụ thuộc message echo/history, chưa được chứng minh. Không mở rộng bảo đảm ngoài dữ liệu thật sự đã ghi.
- Gắn owner/runtime vào mỗi intent là hướng đúng. Cần quy định rõ nơi tạo terminal `NotSent` khi source runtime đã đóng; UI của account mới phải lọc event cũ nhưng source intent không được giả vờ durable.
- Dirty-set metadata cần sequence/version và flush lifecycle; cache sidebar JSON hiện lưu toàn bộ snapshot, nên “chỉ ghi entry tương ứng” có thể cần thay cấu trúc cache hoặc đọc-sửa-ghi nguyên file. Đo trước khi hứa incremental I/O.
- T được chọn từ E1, và sau timeout chỉ có **best-effort client ordering; server order unknown**. Nếu echo không về thì timeout chỉ giúp lane tiếp tục, không giải quyết thứ tự server.

## Trạng thái đồng thuận

Không cần vòng phản biện tĩnh tiếp theo. Có thể bắt đầu bước 1 sau khi người dùng yêu cầu tiếp tục triển khai và chọn semantics logout timeout. Phần còn lại cần xử lý bằng regression/measurement:

1. Logged-out worker → QR/password login → gửi; account A→B; A logout→A login lại khi write cũ đang chạy.
2. Đo clone snapshot, `PersistThreads`, transaction duration, `persist_outgoing` contention và egui frame p95 trên ARM64 fixture.
3. Kiểm thử dirty metadata/cache sau restart, ordering stale writes và logout.
4. Xác nhận reaction enqueue/drain/cursor bằng fixture failure injection.
5. Capture E1 echo/client-ID và phân phối latency trước khi chọn T hoặc khẳng định echo reconciliation.

Không thay đổi giao thức được đề xuất. Chưa có build, test, benchmark hay native acceptance trong review này.
