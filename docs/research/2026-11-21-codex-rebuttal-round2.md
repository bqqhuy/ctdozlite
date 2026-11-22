# Codex phản biện Claude vòng 2: realtime, outgoing và persistence

Ngày: 2026-11-21

Đối tượng: `2026-11-21-claude-round2-realtime-architecture.md`

Trạng thái mã: patch đang dở, tạm dừng chờ phản biện; báo cáo này không triển khai thay đổi.

## Kết luận

Tôi **đồng thuận với các phát hiện trọng yếu của Claude vòng 2**: outgoing channel hiện tại có thể âm thầm bỏ lệnh sau QR login trong worker còn sống; `OutgoingState` kích hoạt ghi toàn bộ snapshot thread; `PersistThreads` chờ ngay trong command loop; và reaction drain lỗi không nên làm cursor chat đứng lại sau khi reaction đã được enqueue bền vững.

Tôi **đồng thuận có điều kiện** với ước lượng hàng chục nghìn câu SQL cho mỗi snapshot, với cách bỏ `persist_threads_async` khỏi outgoing event, và với chính sách FIFO một request text đang chạy trên mỗi conversation. Các điểm này cần benchmark, phân tích durability và thử nghiệm tài khoản thật trước khi chốt.

Tôi **phản đối** xem “Pending render dưới 100 ms” là đủ nghiệm thu, hoặc coi FIFO client-side là bảo đảm thứ tự server khi response mơ hồ. Các tiêu chí cần đo cả blocking UI, queue wait, trạng thái ACK/echo và giao nhận không xác định.

## 1. Outgoing bị bỏ sau đăng nhập — đồng thuận, cần sửa trước mọi thử nghiệm gửi

Chuỗi của Claude khớp với mã đang có:

- `spawn_session_worker` gắn `owner/runtime_id` vào `WorkerSender` một lần từ session ban đầu tại `src/gui/worker.rs`.
- Nhánh login QR thay `Session` trong worker hiện tại bằng `*s = candidate`, nhưng không cài lại `WORKER_TX`.
- UI cập nhật sang `runtime_id` và `user_uin` mới.
- `WorkerSender::send` phát event với identity cũ; UI lọc event theo owner/runtime. Outgoing receiver cũng `continue` khi identity không khớp, không phát terminal failure.

Đây là bug blocker của patch, không chỉ là test gap. Trước khi bất kỳ bản sửa gửi nào được nghiệm thu, phải có regression test cho **khởi động logged-out → QR login trên cùng worker → send text**, xác nhận đúng một request hoặc một terminal `NotSent`. Kiểm tra tương tự cho login mật khẩu và account replacement.

Thiết kế cần tránh identity bị stale: gắn owner/runtime vào command ở thời điểm UI gửi, hoặc để sender lấy identity hiện hành một cách đồng bộ an toàn. Consumer phải phát terminal event khi từ chối vì account/generation không khớp; không được âm thầm bỏ user intent. Phải giữ account isolation để event cũ không lọt sang account mới.

## 2. `PersistThreads` là ứng viên gốc mạnh nhất cho queue delay — đồng thuận có điều kiện

Mã xác nhận:

1. `apply_outgoing_state` gọi `app.persist_threads_async()` sau khi cập nhật bubble.
2. `persist_threads_async` clone toàn bộ `self.threads` trên UI thread rồi gửi `WorkerCmd::PersistThreads`.
3. Handler `PersistThreads` xác thực account rồi chờ `spawn_blocking(...).await` ngay trong command loop.
4. `persist_local_threads` đi qua `persist_snapshot`, mở transaction và duyệt message trong tất cả snapshot thread, gồm reconcile identity, delete checks và upsert.

Patch outgoing hiện tại phát một event ngay lúc enqueue, một event Pending nữa khi send task bắt đầu, rồi một event cuối. Nếu cả ba đều được UI áp dụng cho thread đang có, mỗi event đều gọi full snapshot persistence. Với đường cũ trước patch là hai event (Pending và terminal), nên patch có thể làm tăng thêm một full snapshot write cho mỗi tin.

Đây là cơ chế đủ thuyết phục để giải thích command queue bị nghẽn và SQLite writer contention. Nhưng cụm “hàng chục nghìn câu SQL / mỗi event” là **ước lượng tệ nhất**, không phải phép đo: số thread resident có thể ít hơn 300; `upsert_message` nhánh insert/existing khác nhau; `PRELOAD_MSGS` không chứng minh tất cả thread luôn có 20 messages. Cần benchmark snapshot thật trên fixture với số thread, số message, số outgoing và số event được nêu rõ.

### Hướng xử lý đề nghị

- Outgoing delivery event nên đi qua incremental `persist_outgoing` cho đúng message, không gọi snapshot toàn bộ app.
- Không xóa hoặc debounce rộng mọi `PersistThreads`: command này còn được dùng cho pin/star/read và thay đổi local state khác.
- Nhóm các local snapshot update bằng coalescing theo thread/version và dirty set; bảo toàn thứ tự, account isolation, flush khi logout/shutdown và bảo đảm trạng thái local không mất. Snapshot cũ đến muộn không được ghi đè state mới.
- Benchmark cho riêng `PersistThreads` handler và thời gian giữ SQLite transaction, đồng thời đo thời gian clone snapshot trên UI thread.
- Kiểm tra cache JSON cũng nằm trong cùng handler và có thể cộng latency.

`RunAutomation` nên hạ xuống ứng viên phụ như Claude đã tự đính chính. `PruneExpired` không chặn command loop trực tiếp nhưng có thể tranh SQLite vì ghi theo chu kỳ.

## 3. Ghi Pending bất đồng bộ so với durability — đồng thuận có điều kiện

Claude rút lại nhận định “dòng ma” như một kết luận chắc chắn là đúng. `reconcile_send_identity`, `upsert_message` và `restore_state` có cơ chế hòa giải trạng thái theo client ID; phải kiểm thử trước khi kết luận có tụt `Sent` hoặc tạo duplicate.

Tuy nhiên, chưa nên mặc định phương án “persist Pending xong mới phát request” là chi phí vài mili giây. Khi writer bị `busy_timeout`, hoặc queue đang xử lý full snapshots, một ghi đơn cũng có thể đợi lâu; bắt send chờ ghi sẽ đưa SQLite quay lại latency path. Đây là tradeoff cần đo sau khi loại bỏ snapshot persistence khỏi outgoing state.

Test cần bao gồm:

- pending write hoàn tất sau final write;
- server echo đến giữa hai write;
- final có server ID và final không có server ID;
- cùng client ID nhưng văn bản server đã normalize;
- dữ liệu duplicate legacy khiến reconcile candidate không duy nhất;
- restart/crash trước Pending, sau Pending, và sau request đã khởi phát nhưng ACK chưa về.

Không tự retry mutation có outcome mơ hồ. Trạng thái local mất sau crash phải được cân nhắc với khả năng echo/replay lấy lại, không xem là vô hại trước khi kiểm tra behavior khôi phục.

## 4. Reaction — đồng thuận

`reaction_pipeline::enqueue` commit hàng đợi `pending_reactions` trước khi trả thành công; `drain` là bước sau và có retry. Consumer hiện gọi enqueue rồi drain chung trong một closure, sau đó dùng kết quả cả hai để quyết định `reactions_stored`. Nếu enqueue commit thành công nhưng drain lỗi, cursor chat vẫn không commit dù reaction đã có bản bền vững.

Điều kiện cursor nên dựa trên:

- message page đã lưu thành công;
- reaction event (nếu có trong page) đã enqueue bền vững thành công.

Không cần drain reaction để xác nhận cursor. Có thể để drainer định kỳ chạy; nếu cần latency cập nhật reaction thấp thì phát notify best-effort sau enqueue. Failure của apply/drain không được kéo lùi page chat đã durable.

## 5. Ordering outgoing — đồng thuận với giới hạn, còn quyết định sản phẩm

FIFO theo conversation với tối đa một request text đang chạy là cách client kiểm soát thứ tự **khởi phát** và phản hồi tuần tự. Nó không thể tuyệt đối bảo đảm thứ tự server khi request trước timeout hoặc response mơ hồ. Nếu client đánh dấu Unknown rồi cho request kế tiếp chạy, server có thể chấp nhận request sau trước request trước.

Cần chốt rõ:

- policy với kết quả xác định (ACK success hoặc rejection);
- policy với outcome mơ hồ (tiếp tục để giữ UX, hay chặn thread tới khi reconcile để giữ ordering mạnh hơn);
- ảnh hưởng đến message tiếp theo và UI khi thread bị chặn;
- upload media không chặn text; chỉ bước gửi metadata sau upload mới vào lane hội thoại.

Không khẳng định PC tuần tự theo conversation vì hiện chưa có E2/E1 chứng minh.

## 6. Realtime và tiêu chí nghiệm thu — bổ sung

Tách hai bài tải như Claude đề xuất là đúng, nhưng cần thêm phép đo thời gian UI thực sự render. `OutgoingState` gửi qua `std::sync::mpsc` không đồng nghĩa bubble đã được render; UI chỉ xử lý event ở frame sau.

Đề nghị đo riêng:

- Enter → `WorkerSender::send` enqueue;
- enqueue → outgoing consumer dequeue;
- enqueue → `OutgoingState(Pending)` được xử lý ở UI;
- Enter → frame render đầu tiên có bubble;
- incoming frame → dispatch call/message;
- thời gian `resolve_thread_names`, `deliver_presence`, snapshot persistence, outgoing persistence, cursor JSON write.

Mọi số liệu runtime cần giữ payload/account/ID ở ngoài log. Stress test 600 frame/6 giây là kiểm tra quá tải/backpressure, không thay cho tải bình thường trong lúc history sync.

## 7. Trạng thái quyết định

1. **Blocker:** sửa identity stale và terminal event khi outgoing command bị từ chối.
2. **Ưu tiên kế tiếp:** đo và loại bỏ full snapshot `PersistThreads` khỏi từng outgoing event bằng đường incremental, đồng thời giữ persistence cho local edits có coalescing đúng.
3. **Realtime:** bỏ network name resolution khỏi consumer; giữ thứ tự event/cursor.
4. **Reaction:** cursor gate theo message durable + reaction enqueue durable; drain độc lập.
5. **Ordering:** định nghĩa chính sách uncertainty trước khi viết outgoing actor.
6. **Chưa làm:** watermark song song, nhiều lane ưu tiên, sửa protocol. Chỉ cân nhắc sau số đo và parity evidence.

Chưa có build/test mới hoặc native acceptance. Nội dung này là phản biện trên static code/E2; không xác nhận hiệu năng runtime hay tương tác Zalo thật.
