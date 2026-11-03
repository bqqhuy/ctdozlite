# ADR-0022: Tiếp tục kéo queue control và chẩn đoán sync điện thoại

- **Status:** accepted
- **Date:** 2026-11-03
- **Deciders:** Đợt 1 sửa hồi quy sync iPhone
- **Supersedes:** ADR-0020 về giới hạn 64 cursor và việc chờ phản hồi mới sau cooldown; ADR-0017 về mốc chờ gói 120 giây và vòng đời downloader.

## Context

Log nghiệm thu của bản 76fe200 có lượt danh sách hoàn tất, sau đó 154 mô tả gói message, idx lớn nhất 185 nhưng không có dấu cuối. Descriptor cuối là idx 74, không phải idx lớn nhất. Lần sync thất bại sau control cuối 120,037 giây. Log cũ chưa đủ để chứng minh nguồn mất control là điện thoại hay transport.

Mã r2 áp CatchupGuard cho cả control 603/VoIP 604; cắt sau 64 cursor dù vẫn tiến và không có timer resume. Nó cũng bỏ qua phân trang một số queue được server cấp, ví dụ 610 trong fixture auth. Đây là hồi quy mã đã xác minh; không suy ra 155 control tương ứng đúng 64 trang.

## Decision

Một PullQueues thuộc duy nhất một lần kết nối WS, nhận tất cả queue do auth server cấp (tối đa 256 queue, vượt thì lỗi rõ ràng), không dùng whitelist command để quyết định kéo tiếp. Cursor hợp lệ mới thì tiếp tục không có giới hạn tổng số trang; bộ nhớ detector giữ tối đa 1024 fingerprint gần nhất, không giữ cursor thô trong log. Detector này bắt lặp/quay vòng trong cửa sổ gần nhất, không tuyên bố phát hiện mọi chu kỳ dài tùy ý.

Mỗi queue có một request đang chờ, cursor cuối hợp lệ, deadline và tối đa ba lần đọc lại liên tiếp không có tiến triển. Phản hồi đến muộn có reqId khác request đang chờ không tạo thêm pull. Cursor thiếu/lặp thì chờ 30 giây; không có phản hồi cũng timeout sau 30 giây. Timer 100 ms xử lý tối đa tám queue đến hạn mỗi nhịp, phát lại cùng cursor, không tạo yêu cầu sync điện thoại mới. Cursor tiến đặt lại ngân sách retry. Queue hoàn tất hủy lịch; disconnect hủy toàn bộ scheduler và request không replay qua epoch mới.

HTTP giữ polling định kỳ hiện có, không có page cap 64; cursor lặp nghỉ 30 giây rồi polling lại từ cùng state. Không triển khai cursor bền vững hay ACK gắn transaction ở đợt này.

Control frame chỉ chứa transfer_sync2 đi qua broadcast nhận sync và không bị chặn bởi mailbox UI 32 phần tử. Frame trộn call/control khác vẫn được chuyển GUI đầy đủ. Receiver sync vẫn có giới hạn và báo lỗi cụ thể khi broadcast lag, không âm thầm coi đầy đủ.

Mốc chờ gói tăng lên 300 giây hoạt động, xác nhận vẫn 180 giây, toàn phiên 1800 giây; Idle vẫn tạm dừng bộ đếm hoạt động. Đây là lựa chọn thử nghiệm của fork, không coi tăng timeout là sửa gốc. Hủy đóng/drain staging, join receiver/downloader có hạn, rồi dispose; Drop dự phòng abort task. Writer hoàn tất transaction đang chạy, không cắt nửa transaction. Không mở lại lượt tự động hoặc đổi coverage.

UI tách đã nhận mô tả/idx lớn nhất/lỗ hổng/dấu cuối khỏi tải byte. Sau nhập gói chuyển sang chờ, không giữ nhãn giải mã. Đã xác nhận nhưng chưa có batch được báo riêng. Log có round/cmd/subcmd, quyết định queue/epoch/retry, bộ đếm thực của receiver, tải/decode/resolve-sender/import begin/end/aborted và thời lượng, dispose socket-write ACK. ACK ghi socket không phải xác nhận điện thoại đã dừng.

Media log tách lỗi chuẩn bị, gửi request, mạng/HTTP, mã API ngoài/lồng, phân tích phản hồi, cache hit. Không ghi URL/khóa/body, timestamp scope hay cursor thô. Nếu mailbox log quá tải có sự kiện diagnostic_dropped, tránh nhầm mất log thành mất gói.

## Verification and limits

Test >400 trang cho 603/604/510/511/610; 600 batch đảo thứ tự trên 300 frame control đi qua encode/decode và Round; repeat/cycle/missing cursor; timer không cần push mới; hết ba retry; response timeout; stale reqId; completion/reconnect; thiếu dấu cuối; hủy/join rồi tạo lượt mới. Phép thử transport/round là fixture trong bộ nhớ, không tuyên bố đã nghiệm thu iPhone hay chạy container crypto thực trong toàn luồng đó.

Không thay schema 8, thuật toán crypto, phạm vi request hoặc điều kiện đủ mọi idx tới isLast. Chưa làm rePrepare/coverage theo scope (Đợt 2). Cần nghiệm thu tài khoản thật, tin cũ chỉ còn trên iPhone và so lượng nhập với baseline.
