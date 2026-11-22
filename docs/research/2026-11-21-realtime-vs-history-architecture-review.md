# Phản biện kiến trúc realtime và đồng bộ lịch sử của ZaloLite

Ngày: 2026-11-21

Phạm vi: lỗi gửi/nhận tin, file/ảnh/video, cuộc gọi, WebSocket, SQLite và đồng bộ lịch sử

Mục đích: cung cấp một bản độc lập để Claude phản biện trước khi tiếp tục sửa kiến trúc.

## 1. Kết luận cần phản biện

ZaloLite đang trộn nhiều loại công việc có độ trễ và mức ưu tiên khác nhau vào worker điều phối chung. Đường gửi text đã được tách riêng bước đầu, nhưng đường nhận realtime vẫn có thể chờ các tác vụ chậm như resolve tên, SQLite persistence và reaction reconciliation trước khi đọc frame WebSocket tiếp theo.

Hướng đúng là tách thành hai mặt phẳng:

```text
Realtime plane: WebSocket receive → parse → dispatch tin/call/receipt → UI state
Durability plane: ordered SQLite/cache/reaction/cursor writer

History sync plane: phone sync, full history, refresh, metadata (priority thấp)
Media plane: upload/download/decode/renew URL (bounded, độc lập)
Outgoing plane: text/file/image/video/voice mutation và ACK reconciliation
```

Tin nhắn đến/đi và tín hiệu cuộc gọi phải được ưu tiên cao hơn sync lịch sử. Tuy vậy, ưu tiên cao không được đồng nghĩa với bỏ qua lưu bền vững hoặc tăng cursor trước khi dữ liệu được lưu thành công.

## 2. Triệu chứng người dùng đã báo

### Gửi

- Nhấn Enter nhưng bubble phải vài giây, thường nhanh nhất khoảng 5 giây, mới xuất hiện.
- Sau tin đầu tiên, các tin gửi liên tiếp thường đứng hoặc báo `Hết ngân sách chờ QueueWait`.
- Zalo PC gốc gửi liên tục mượt; fork dễ hết queue wait.
- Tin/file/ảnh/video và voice cần được xem là các loại mutation khác nhau nhưng không được chặn nhau vô thời hạn.

### Nhận và nhóm

- Nhóm/cộng đồng có thể treo ở “đang xác minh quyền gửi”.
- Tên danh bạ trong group không đồng bộ đầy đủ.
- Reaction hiển thị chung chung, thiếu tên người gửi và có vẻ lấn át tin nhắn.
- Group/avatar mặc định không giống Zalo PC.

### Giao diện liên quan đến độ trễ

- Dải đen xuất hiện khi nhập ô tìm kiếm hoặc khi có dòng sync.
- Khung soạn thảo cách đáy nhiều.
- Emoji/heart và khoảng cách glyph khác xa Zalo gốc.

Các lỗi UI này có thể độc lập, nhưng lỗi “delay” của bubble có quan hệ trực tiếp với thời điểm worker phát `GuiEvent::OutgoingState`.

## 3. Phát hiện trong fork

### 3.1. Đường gửi cũ

Trong `src/outgoing.rs`, fork từng làm ba việc nguy hiểm:

1. Ghi nhận thời điểm enqueue.
2. Nếu queue chờ quá `QUEUE_BUDGET = 5s`, biến queue wait thành lỗi `SendBudgetElapsed(QueueWait)` và dừng trước transport.
3. Tính `TEXT_TOTAL_BUDGET = 20s` từ lúc enqueue, nên thời gian nằm trong command queue đã ăn vào ngân sách network.

Điều này giải thích trực tiếp lỗi người dùng thấy: một tin có thể chưa hề được gửi lên server nhưng đã bị fork đánh dấu lỗi do chậm ở local dispatcher.

Đã sửa bước đầu:

- `WorkerSender` có kênh outgoing riêng.
- Gửi text không phải chờ command dispatcher chung.
- UI nhận trạng thái pending optimistic ngay sau enqueue.
- Queue wait chỉ còn là diagnostic metric.
- Ngân sách 20 giây bắt đầu khi worker bắt đầu send.

Đây là sửa đúng hướng nhưng chưa đủ cho incoming/realtime.

### 3.2. WebSocket receive loop vẫn làm việc tuần tự

Trong `src/gui/worker.rs`, WebSocket loop hiện có thể thực hiện liên tiếp:

```text
nhận WsEvent
→ parse VoIP
→ parse reaction
→ mở SQLite / enqueue / drain reaction
→ merge chat payload
→ resolve_thread_names (network/read)
→ persist_received (spawn_blocking SQLite)
→ publish_live
→ commit_socket cursor
→ mới nhận frame kế tiếp
```

Các bước persistence có chủ ý giữ cursor an toàn, nhưng đang nằm trên đường đọc event. Nếu reaction hoặc SQLite chậm, frame tin nhắn đến sau và control call phải chờ.

### 3.3. Sync/history là command/task riêng nhưng cùng runtime

Fork đã có các command như `SyncHistory`, `SyncPhoneHistory`, `PollMessages`, `LoadGroupHistory`, cùng các task nền. Đây là nền tảng tốt, nhưng cần bảo đảm chúng không chiếm quyền xử lý của realtime ingress, outgoing và call control.

### 3.4. Media đã có queue riêng từng phần

Ảnh/sticker có media job queue; upload có progress và resumable state; file drop/voice có task riêng. Tuy nhiên cần kiểm tra lại toàn bộ đường file/image/video để bảo đảm upload, decode, thumbnail và renew URL không đi xuyên qua command queue chung hoặc giữ mutex/session lâu.

## 4. Zalo PC gốc làm gì theo bằng chứng hiện có

Các kết luận dưới đây dựa trên bundle Zalo PC/Web đã lưu và `re-notes/`; đây là E2 static evidence, không phải benchmark runtime live.

### 4.1. WebSocket là realtime transport

Bundle PC có `config.zpw_ws` và các lớp queue/control:

- Chat push/pull: `501`, `521`, `510`, `511`, `513–518`, `570`.
- VoIP/control: `601–604`.
- Heartbeat/connection management: các command riêng.

Kênh WebSocket không phải worker đồng bộ toàn bộ lịch sử. Nó nhận push và pull theo queue, rồi chuyển dữ liệu vào các handler phù hợp.

### 4.2. Sync lịch sử dùng sync-v2 worker

Zalo PC/Web có `sync-v2-worker` và `sync-v2-sub-worker` cho phone/history synchronization. Bundle cho thấy:

- Công việc chia theo conversation và batch.
- Batch có `idx`, `isLast`, `totalBatch`.
- Có `batch_timeout`, retry và điều kiện lấy lại scope chưa hoàn tất.
- `max_number_of_concurrent_sync_items` mặc định quan sát được là 4.
- Có wake-up/dispose flow riêng khi điện thoại idle hoặc reconnect.

Đây là công việc dài, có thể kéo hàng phút, nên không thể dùng chung latency path với một tin chat hoặc cuộc gọi.

### 4.3. Cursor phân biệt received và processed

Static audit `getLastActionIdsForSocket`, `addLastActionIds`, `__markWaitId`, `saveData` cho thấy PC phân biệt tiến độ nhận với tiến độ xử lý/lưu. `lastId` đi kèm tối đa 10 `preIds`; equal `lastId` không chứng minh hai page giống nhau.

Các ghi chú queue audit của fork cũng đã xác lập nguyên tắc an toàn:

- Chỉ commit cursor sau khi page được lưu.
- Nếu ghi SQLite thất bại, giữ cursor ở durable point cũ để replay.
- Một queue lỗi không được làm mất progress của queue khác.

### 4.4. Media là pipeline bất đồng bộ

Bundle PC cho thấy file/video upload có chunk và control event `file_done`/`video_done`; chỉ sau khi có URL/fileId client mới gửi message metadata. Video còn tách video URL và poster/thumbnail. Forward media ưu tiên `renewLink`, có fallback re-upload.

Điều này tạo ra lifecycle riêng:

```text
prepare → upload/download/renew → media done → send metadata → ACK/echo → render
```

### 4.5. VoIP control cần dispatch nhanh

VoIP control frames được xử lý qua call state/native bridge, tách khỏi history sync. Một cuộc gọi đến không được chờ reaction drain, avatar fetch, full history hoặc SQLite batch rồi mới báo UI.

## 5. Những gì đã biết chắc và những gì chưa được chứng minh

### Đã có bằng chứng đủ mạnh

- PC có WebSocket realtime riêng và sync-v2 worker riêng.
- PC chia queue chat/control, không coi toàn bộ dữ liệu là một batch history.
- PC có cursor/processed distinction và replay khi chưa durable.
- Media upload/video done là asynchronous.
- Fork biến QueueWait thành send failure là nguyên nhân cục bộ rõ ràng cần loại bỏ.

### Chưa được phép khẳng định

- Chưa có capture E1 chứng minh scheduler thread priority hoặc một con số “tin nhắn luôn được ưu tiên tuyệt đối” trong PC.
- Chưa có live capture đủ để chốt mọi schema reaction/group/member-name.
- Chưa được suy ra server contract mới chỉ từ tên route hoặc command index.
- Chưa có benchmark ARM64 chứng minh độ trễ trước/sau.

Vì vậy, “ưu tiên tuyệt đối” nên được triển khai ở boundary nội bộ của fork: realtime ingress và outgoing không bị block bởi history sync; không nên tuyên bố đó là một tham số scheduler đã được xác minh của Zalo PC.

## 6. Hướng kiến trúc đề xuất cho fork

### 6.1. Realtime ingress

WebSocket reader chỉ làm công việc ngắn và bounded:

```text
frame bytes → decrypt/decode → classify → dispatch event
```

Nó không được chờ:

- SQLite.
- Resolve profile/name qua network.
- Reaction drain.
- Full history fetch.
- Media download/decode.

Event được đưa vào các queue có giới hạn:

- `incoming_message_tx`
- `receipt_presence_tx`
- `call_control_tx`
- `reaction_tx`
- `durable_page_tx`

Call control và receipt có ưu tiên cao nhất. Tin nhắn đến được phát UI optimistic ngay khi parse hợp lệ, nhưng trạng thái durable/cursor vẫn chờ writer xác nhận.

### 6.2. Ordered durable writer

Một writer task per account xử lý theo thứ tự page/queue:

```text
message batch → filter deleted → SQLite transaction → cache snapshot → cursor commit
```

Writer phải trả kết quả terminal:

- `StoredAndCommitted`.
- `StoredCursorDeferred`.
- `PersistenceFailedAndCursorRetained`.

Nếu writer chậm, incoming UI vẫn cập nhật; nếu writer lỗi, cursor không tiến và page sẽ replay. Queue writer phải bounded để không tạo memory leak khi SQLite hỏng lâu.

### 6.3. Outgoing scheduler

Outgoing cần identity/idempotency riêng cho từng intent:

- Text/quote/mention/sticker: gửi qua outgoing channel, optimistic pending, ACK/echo reconcile.
- File/image/video/voice: media prepare/upload queue riêng, sau đó enqueue message metadata.
- Cùng một conversation giữ ordering cần thiết.
- Hai conversation khác nhau không được khóa lẫn nhau.
- Timeout không tự suy ra “chưa gửi” nếu mutation có thể đã đến server.

### 6.4. Media scheduler

Media jobs có semaphore và progress riêng:

- Giới hạn số upload/download/decode đồng thời.
- Không giữ session mutex qua network await nếu có thể snapshot immutable session.
- Không để thumbnail hoặc preview chiếm slot của upload message.
- Hủy theo account/runtime generation.

### 6.5. History/sync scheduler

History sync chạy priority thấp:

- Có cancellation token.
- Có concurrency cap.
- Có thể pause/yield khi outgoing hoặc call control đang hoạt động.
- Không gửi trực tiếp `ThreadsUpdated` làm ghi đè snapshot realtime nếu generation đã cũ.
- Không dùng chung queue/cursor với realtime message queue.

## 7. Invariants bắt buộc

1. WebSocket reader không chờ SQLite/network phụ ngoài bước decode cần thiết.
2. Incoming message UI update không chờ full history sync.
3. Outgoing text UI pending không chờ sync/search/avatar/reaction.
4. Call invite/answer/end không chờ persistence hoặc media thumbnail.
5. Cursor chỉ commit sau durable write thành công.
6. Persistence fail giữ cursor cũ và cho phép replay.
7. Một queue lỗi không chặn queue chat/control khác.
8. Retry mutation chỉ dùng khi chứng minh pre-send failure hoặc có idempotency an toàn.
9. Mọi queue có giới hạn; không spawn vô hạn task theo số frame.
10. Account/runtime generation phải được kiểm tra trước khi áp dụng event muộn.

## 8. Kế hoạch triển khai đề xuất

### P0 — đo và quan sát trước khi tách lớn

- Ghi timing sanitized cho từng đoạn: WebSocket receive, parse, dispatch, profile resolve, SQLite, cursor commit, UI event.
- Không ghi nội dung tin, account, cookie hoặc URL.
- Tạo fixture synthetic cho một burst chat + reaction + call control + history task.

### P1 — tách ingress khỏi persistence

- Giữ thứ tự event bằng một durable queue bounded.
- WebSocket loop chỉ classify/dispatch.
- Writer task xử lý persist/cursor tuần tự.
- Test page replay khi writer fail.

### P2 — tách priority lanes

- Lane call control/receipt.
- Lane incoming chat.
- Lane outgoing.
- Lane media.
- Lane history/sync.

Đặt fairness để history không chết đói, nhưng không được phép chiếm lock/worker khiến realtime chờ vô hạn.

### P3 — kiểm tra media và UI

- Đo upload file/ảnh/video trong khi nhận burst tin.
- Kiểm tra avatar/name resolution chạy ngoài ingress.
- Sửa UI search/sync banner sau khi timing path ổn định; không dùng UI patch để che queue delay.

## 9. Tiêu chí nghiệm thu

### Unit/fixture

- Burst 20 outgoing text không sinh `QueueWait` failure giả.
- Incoming frame tiếp theo vẫn được dispatch khi SQLite writer bị delay.
- Call control được dispatch trước reaction drain/history persistence.
- Cursor không tăng khi writer fail.
- Queue A fail không chặn queue B.
- Media upload đang chạy không làm chậm outgoing text.
- Stale generation event bị loại bỏ sau logout/login.

### Native Windows ARM64

- Đo p50/p95 từ Enter đến pending bubble.
- Đo p50/p95 từ frame nhận đến bubble incoming.
- Nhắn liên tục trong khi history sync đang chạy.
- Gửi file/ảnh/video trong khi nhận tin và có cuộc gọi control.
- Reconnect WebSocket giữa chừng và kiểm tra replay/cursor không mất tin.

Unit test không thay thế được nghiệm thu tài khoản Zalo thật.

## 10. Câu hỏi bắt Claude phản biện

1. Có bằng chứng nào cho thấy `src/gui/worker.rs` còn để SQLite/name resolution/reaction drain trên WebSocket receive path không? Hãy chỉ rõ dòng và tác động.
2. Đề xuất ingress queue nào giữ được ordering và cursor safety mà không làm tăng memory vô hạn?
3. Có cần một writer per account, per queue hay một writer toàn worker? Trade-off là gì?
4. Làm thế nào chứng minh call control và outgoing không bị starvation khi history sync chạy?
5. Những phát biểu nào trong tài liệu này là E2 static, E1 live capture hay E3 inference? Hãy đánh dấu lại nếu cần.
6. Có thay đổi nào vi phạm invariant “cursor chỉ commit sau durable write” hoặc làm duplicate mutation?
7. Cách đo latency trên Windows ARM64 nào phản ánh đúng trải nghiệm người dùng mà không đưa secret/nội dung chat vào log?
8. Có cần sửa protocol không, hay chỉ cần sửa scheduling/ownership nội bộ? Không được đề xuất đổi command/param nếu chưa có E1/E2 phù hợp.

## 11. Tài liệu nguồn

- `docs/ZALO_REFERENCE_ANALYSIS.md`
- `re-notes/02_chat_data.md`
- `re-notes/17-zalo-pc-sync-config-and-reconnect.md`
- `re-notes/18-typing-receipts-and-pr78.md`
- `re-notes/21-queue-cursor-audit.md`
- `re-notes/22-refresh-durable-baseline-audit.md`
- `re-notes/44-map15-network-send.md`
- `src/gui/worker.rs`
- `src/outgoing.rs`
- `src/queue_cursors.rs`
- `src/socket.rs`

## 12. Giới hạn hiện tại

Đây là bản phân tích và đề xuất kiến trúc, chưa phải tuyên bố đã đạt parity runtime với Zalo PC. Cargo test/build trên máy hiện bị chặn bởi dependency `ring` không tìm thấy `clang.exe`; native ARM64 live acceptance và kiểm thử tài khoản thật chưa được thực hiện trong bản này.
