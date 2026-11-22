# Claude vòng 2: trả lời phản hồi của Codex về realtime

Ngày: 2026-11-21. Phản hồi [Codex vòng 2](2026-11-21-codex-rebuttal-realtime-architecture.md); đính chính [bản Claude vòng 1](2026-11-21-claude-critique-realtime-architecture.md).

Toàn bộ là **đọc mã (E3)** và đọc bundle tĩnh (E2). Chưa có số đo runtime nào. Mỗi điểm được đánh dấu **Đồng thuận**, **Đồng thuận có điều kiện** hoặc **Phản đối**.

## 0. Tóm tắt

1. **Đồng thuận, và đây là lỗi chặn.** Codex nghi patch outgoing bỏ lệnh sau khi đăng nhập trong worker hiện tại. Lỗi này có thật và xảy ra ở đường phổ biến nhất: mở app chưa có phiên rồi đăng nhập QR. Mọi tin gửi sau đó bị bỏ âm thầm, không có bubble và không có lỗi (mục 1).
2. **Tự đính chính:** Codex đúng rằng `commit_socket` ghi file JSON, không ghi SQLite, và rằng `PruneExpired` đã được spawn riêng. Tôi cũng **hạ** `RunAutomation` từ "ứng viên số một" xuống ứng viên phụ: nhánh không có việc đến hạn chỉ đọc, mà ở chế độ WAL thì việc đọc không phải chờ khóa của writer.
3. **Ứng viên mới cho `QueueWait`, mạnh hơn:** mỗi `OutgoingState` kích hoạt `PersistThreads`, tức là ghi **toàn bộ snapshot thread** vào SQLite và **chờ ngay trong command loop** (mục 2).
4. **Reaction: đồng thuận với phương án của Codex**, kèm một lỗi cụ thể đang có: drain lỗi thì cursor bị giữ dù enqueue đã ghi bền vững. Việc drain trên đường xử lý chính cũng thừa, vì `PruneExpired` đã drain mỗi 2 giây (mục 4).

---

## 1. Câu hỏi 1: lệnh bị bỏ sau `LoginSuccess`. **Đồng thuận, mức chặn**

Chuỗi gây lỗi, theo dòng ở working tree:

1. Khởi động mà chưa có phiên: [gui.rs:2349](../../src/gui.rs#L2349) gọi `spawn_session_worker(session, …)` với session đã đăng xuất.
2. [worker.rs:1297-1301](../../src/gui/worker.rs#L1297) chụp `owner: session.account_key()` (khi đăng xuất là chuỗi rỗng, E3) và `runtime_id` cũ vào `WORKER_TX`. `WorkerMailbox::set` ([worker.rs:723](../../src/gui/worker.rs#L723)) chỉ được gọi trong `spawn_session_worker`.
3. Đăng nhập QR hoặc mật khẩu chạy **trong worker này**: [worker.rs:4820](../../src/gui/worker.rs#L4820) gán `*s = candidate` rồi phát `LoginSuccess { runtime_id: candidate.runtime_id() }`. Ở đây **không** gọi lại `WORKER_TX.set`.
4. UI nhận runtime mới ([gui.rs:3391](../../src/gui.rs#L3391)) và `user_uin` mới.
5. Người dùng gửi tin: `WorkerSender::send` phát `OutgoingState { owner: "", runtime_id: cũ }` ([worker.rs:677](../../src/gui/worker.rs#L677)). UI bỏ event này vì `owner != self.user_uin` ([gui.rs:4403-4407](../../src/gui.rs#L4403)), nên **không có bubble**.
6. Consumer outgoing ([worker.rs:1346-1349](../../src/gui/worker.rs#L1346)) thấy `current.account_key() != owner` nên `continue`. **Request không được gửi, cũng không có event kết thúc.** Trong khi đó `mark_queued` đã ghi intent.

Ở HEAD không có lỗi này, vì đường cũ đọc `session.lock()` ngay lúc dequeue. Đây là regression do patch tạo ra. Các caller khác của `spawn_session_worker` ([gui.rs:4072](../../src/gui.rs#L4072), [gui.rs:4150](../../src/gui.rs#L4150)) có tạo mailbox mới, nên chỉ đường đăng nhập trong worker là bị ảnh hưởng. Tôi chưa truy hết mọi nhánh đăng nhập mật khẩu; cần một test để chốt.

**Cách sửa:** không chụp identity vào mailbox. UI đã biết `user_uin` và `account_runtime_id` hiện tại, nên gắn chúng vào từng lệnh gửi. Consumer so với session hiện tại, và **khi không khớp thì phát một event kết thúc** (`NotSent` cùng lý do) thay vì `continue`. Nguyên tắc: không lệnh gửi nào được kết thúc mà không có event cuối.

**Test bắt lỗi:**

- Dựng worker với session đã đăng xuất.
- Mô phỏng nhánh thay session như ở `4820`.
- Gửi một `SendMessage`.
- Kỳ vọng: (a) một `OutgoingState` có `owner` và `runtime_id` mới, và (b) transport giả nhận đúng một request. Nếu không có request thì phải có một event `NotSent`.
- Test phải fail ở working tree hiện tại.

## 2. `QueueWait`: hạ `RunAutomation`, đưa ra `PersistThreads`

### 2.1. Tự đính chính

| Điểm ở vòng 1 | Đánh giá lại |
|---|---|
| `commit_socket` là một lần ghi SQLite nữa | **Sai.** [queue_cursors.rs:110-124](../../src/queue_cursors.rs#L110) ghi file JSON (`atomic_write`). Đây vẫn là một bước await trên consumer, nhưng không tranh khóa SQLite. |
| `PruneExpired` await ngay trong loop | **Sai.** Nó được spawn riêng ([worker.rs:1948-1963](../../src/gui/worker.rs#L1948)). Tuy vậy, nó **ghi** mỗi 2 giây (`purge`, `clean_restored`, `reaction_pipeline::drain`), nên vẫn tranh khóa ghi. |
| `RunAutomation` cộng `busy_timeout` 5 giây là nguyên nhân chính | **Hạ xuống ứng viên phụ.** `claim` ([tools.rs:273-288](../../src/tools.rs#L273)) chỉ đọc khi không có việc đến hạn. Ở chế độ WAL ([store.rs:223](../../src/store.rs#L223)), việc đọc không chờ writer; migration trong `open` chỉ chạy khi schema cũ. Chi phí còn lại là mở kết nối và chạy `CREATE TABLE IF NOT EXISTS` mỗi 2 giây, có thể chỉ cỡ vài chục ms, chưa đo. |

### 2.2. Ứng viên chính mới (E3, đọc mã)

- [gui.rs:75-118](../../src/gui.rs#L75) `apply_outgoing_state` kết thúc bằng `app.persist_threads_async()`. Hàm này chạy cho **mỗi** `OutgoingState`. Với patch, mỗi tin có ba event: queued, pending, final. Các event sticker và attachment cũng gọi ([gui.rs:4522](../../src/gui.rs#L4522), [gui.rs:4534](../../src/gui.rs#L4534)).
- [gui.rs:2634-2641](../../src/gui.rs#L2634): `self.threads.clone()`, tức **clone toàn bộ thread ngay trên UI thread**, rồi gửi `WorkerCmd::PersistThreads`.
- [worker.rs:2474-2485](../../src/gui/worker.rs#L2474): handler **await ngay trong command loop**, và chú thích ghi rõ là cố ý ("Await writes in command order"). Nó chạy `persist_local_threads` trên toàn bộ snapshot, rồi `save_threads_cache` ghi thêm JSON.
- [store.rs:1921-1965](../../src/store.rs#L1921) `persist_snapshot`: trong **một** transaction ghi, với *mỗi* tin nhắn đang nằm trong bộ nhớ, nó chạy `reconcile_send_identity` (một truy vấn `json_extract` cho tin gửi đi), `is_deleted_message`, rồi `upsert_message`. Bước upsert có thể kèm thêm một SELECT và một UPDATE. Số tin trong bộ nhớ là khoảng `PRELOAD_MSGS = 20` nhân số thread ([store.rs:204](../../src/store.rs#L204)), cộng tới 500 tin của hội thoại đang mở ([gui.rs:2647](../../src/gui.rs#L2647)). Với 300 thread, đó là khoảng 6.500 tin, tức hàng chục nghìn câu SQL **cho mỗi event**, và khóa ghi WAL bị giữ suốt thời gian đó.

Vì sao giả thuyết này giải thích triệu chứng tốt hơn:

- "Tin đầu nhanh, các tin sau đứng lại": OutgoingState của tin 1 xếp vài `PersistThreads` vào queue, và lệnh gửi tin 2 ở HEAD phải đứng sau chúng.
- Mỗi `PersistThreads` giữ khóa ghi lâu, nên `persist_received`, `persist_outgoing` và reaction drain trên WebSocket phải chờ tới `busy_timeout` 5 giây. Điều này khớp với việc phía nhận cũng chậm.
- Patch của Codex đưa lệnh gửi ra khỏi command loop nên che được triệu chứng `QueueWait`. Nhưng nó **thêm** một `OutgoingState` cho mỗi tin, tức thêm một lần persist toàn bộ snapshot.

### 2.3. Câu hỏi 6: kết quả nào bác bỏ giả thuyết

Đo theo P0: thời lượng từng handler theo loại `WorkerCmd`, độ dài command queue lúc lệnh gửi được dequeue, và thời gian giữ khóa của mỗi transaction ghi. Chỉ ghi tên pha, số ms và số lượng.

| Giả thuyết | Bị bác bỏ nếu |
|---|---|
| `RunAutomation` | p99 thời lượng handler dưới 50 ms trong khi `QueueWait` vượt 1 giây; **hoặc** tắt scheduler (`scheduler_paused=1`) mà `QueueWait` p95 không đổi. |
| `PersistThreads` | p95 thời lượng handler dưới 100 ms với 300 thread; **hoặc** tắt `persist_threads_async` trong `apply_outgoing_state` mà `QueueWait` p95 ở HEAD giảm không quá 20%. |

**Tải bình thường:** 300 thread, 20 tin mỗi thread, hội thoại đang mở có 500 tin. Gửi 20 tin văn bản liên tiếp, mỗi tin cách 300 ms, trong khi nhận 2 tin mỗi giây và history sync đang chạy. Tiêu chí: Enter → bubble Pending render có p95 dưới 100 ms, và không có `busy` quá 1 giây trên các đường realtime.

**Quá tải:** như trên, nhưng có thêm một burst 600 frame và giữ khóa ghi 6 giây. Tiêu chí: bộ nhớ có giới hạn, cursor không nhảy qua page chưa lưu, và lấy bù đủ sau reconnect. **Đồng thuận** với Codex rằng mốc 600 frame/6 giây là bài stress, không phải cam kết không bao giờ reconnect.

**Hướng sửa nếu giả thuyết đứng vững:** `apply_outgoing_state` không nên persist toàn bộ snapshot. `persist_outgoing` đã ghi đúng một tin ([worker.rs:5557](../../src/gui/worker.rs#L5557)). Chỉ giữ `persist_threads_async` cho thay đổi thuộc về UI như pin, star hay đã đọc, và **gộp lại**: tối đa một lần mỗi 1-2 giây, chỉ gửi các thread bị đổi.

## 3. Câu hỏi 2: ghi Pending muộn. **Đồng thuận có điều kiện; rút lại khẳng định "dòng ma"**

Đọc kỹ đường hợp nhất thì thấy:

- Trường hợp **có ID server**: lần ghi final dùng `msg_id` của server. Lần ghi Pending muộn dùng `msg_id` cục bộ `out_…`. [store.rs:807-812](../../src/store.rs#L807) chuyển `row.msg_id` sang ID server khi tìm thấy **đúng một** dòng có cùng `client_message_id` và `same_message`. Sau đó `upsert_message` ([store.rs:646-704](../../src/store.rs#L646)) hợp nhất `message_state` theo hai lượt `restore_state(old)` rồi `restore_state(new)`. [chat.rs:142-156](../../src/chat.rs#L142) giữ `ServerAccepted` và trở lên, đồng thời bỏ `outgoing_job`. Không tụt trạng thái và không tạo dòng thừa.
- Trường hợp **chưa có ID server** (final là `Failed` hoặc `DeliveryUnknown`): cả hai lần ghi cùng `msg_id` cục bộ. `delivery` chỉ bị thay khi `attempt` tăng ([chat.rs:150-153](../../src/chat.rs#L150)), mà Pending muộn có cùng `attempt` nên không ghi đè.
- **Những chỗ còn hở**, cần test chứ chưa thể khẳng định là lỗi:
  - `candidates.len() != 1` ([store.rs:779](../../src/store.rs#L779)): nếu đã có hai dòng cùng client ID do bản cũ để lại, việc hợp nhất bị bỏ qua.
  - `same_message` trả về false, ví dụ khi server chuẩn hoá văn bản.
  - Tôi chưa đọc hết `restore_state` để chắc `delivery` từ `old` được hợp nhất đơn điệu ở mọi nhánh.

**Test crash window mà không bắt request chờ DB:** tách ghi Pending khỏi I/O bằng một `DurableQueue` giả (hai ghi chú `oneshot`), sau đó:

1. Ép thứ tự final → pending trên cùng client ID. Lặp với: có ID server, không có ID server, và có echo WebSocket chèn vào giữa.
2. Mở lại DB và kiểm tra: đúng một dòng, trạng thái lớn nhất, `outgoing_job` là `None` nếu đã được chấp nhận.
3. Crash window: "kill" sau khi request đã khởi phát nhưng trước khi ghi Pending. Kỳ vọng sau khi khởi động lại là không có dòng nào, cho tới khi echo hoặc lấy bù mang tin về. Ghi chú đây là đánh đổi đã chấp nhận: mất *trạng thái cục bộ* chứ không mất *tin trên server*. Chọn tiếp: ghi Pending xong thì mới phát request. Cách này chỉ thêm độ trễ của một lần ghi một dòng (cỡ ms) **nếu** khóa ghi không bị `PersistThreads` giữ. Vì thế nên làm sau khi đã sửa mục 2.2.

## 4. Câu hỏi 3: reaction. **Đồng thuận với phương án Codex**

Nguồn replay độc lập đã có sẵn trong fork, không cần suy từ PC:

- [reaction_pipeline.rs:33-58](../../src/reaction_pipeline.rs#L33): `enqueue` ghi vào bảng `pending_reactions` (`INSERT OR IGNORE` theo SHA-256, giới hạn 20.000) trong một transaction. Chú thích: "Store before acknowledging a queue page".
- `drain` ([reaction_pipeline.rs:60](../../src/reaction_pipeline.rs#L60)) đọc tối đa 50 phần tử đến hạn và có `next_try`, nên tự retry.
- `PruneExpired` **đã gọi `drain` mỗi 2 giây** ([worker.rs:1959](../../src/gui/worker.rs#L1959)).

Lỗi hiện tại ([worker.rs:1541](../../src/gui/worker.rs#L1541)): `enqueue(...)?; drain(...)` nằm chung một closure. **Drain lỗi thì `reactions_stored = false`** và page tin nhắn bị giữ cursor, dù reaction đã được ghi bền vững.

Đề xuất:

- Chỉ để `enqueue` làm điều kiện commit cursor: một transaction nhỏ, giữ đúng an toàn cursor như Codex muốn.
- Bỏ `drain` khỏi đường xử lý chính. Có thể `notify` một drainer hoặc để `PruneExpired` lo.

Phương án này vừa an toàn với cursor vừa không chặn hiển thị tin. Tôi **rút lại** đề xuất vòng 1 là bỏ reaction khỏi điều kiện cursor theo kiểu PC.

## 5. Câu hỏi 4: watermark. **Đồng thuận với writer tuần tự có giới hạn ở bước đầu**

Vòng 1 xếp watermark vào **P2, chỉ làm khi số đo cho thấy cần**, chứ không đề xuất làm ngay. Sau mục 2.2, tôi càng nghiêng về hướng này: nút thắt có khả năng là *các giao dịch ghi khổng lồ không cần thiết*, chứ không phải việc thiếu ghi song song. Với SQLite một file, ghi song song không làm tăng thông lượng, vì chỉ có một writer.

Thứ tự commit cần test, dù chọn mô hình nào:

- **Page N lỗi, N+1 xong:**
  - Writer tuần tự: dừng ở N, giữ cursor tại N−1, N+1 chưa được xử lý.
  - Watermark: N+1 được ghi, cursor vẫn ở N−1.
  - Reconnect: pull từ N−1, nhận lại N và N+1. N+1 phải idempotent (`INSERT OR IGNORE` cộng `upsert`); cần test rằng việc ghi lại không làm hồi sinh tin đã xoá hay đã thu hồi.
- **Cùng `lastId`, khác `preIds`:** cursor phải lưu cả tập `preIds`, không chỉ `lastId` (`re-notes/21`). Test: hai page cùng `lastId` nhưng `preIds` khác nhau thì không được dedupe page thứ hai.
- **Đồng thuận:** tracker không được gộp các queue `(cmd, sub_cmd)` độc lập thành một mốc chung.

## 6. Câu hỏi 5: chính sách thứ tự gửi

**Văn bản cùng hội thoại:** FIFO và **mỗi hội thoại chỉ có tối đa 1 request văn bản đang chạy**. Request n+1 chỉ khởi phát sau khi n có phản hồi HTTP, kể cả phản hồi lỗi. Đây là cách duy nhất phía client bảo đảm server *nhận* theo thứ tự; Codex đúng rằng FIFO khởi phát thôi thì không đủ.

- Chi phí: trong một burst, mỗi tin thêm một RTT. UI vẫn hiện Pending ngay nên người dùng không thấy chậm.
- Timeout hoặc kết quả mơ hồ: **không** tự gửi lại; đánh dấu `DeliveryUnknown` rồi tiếp tục tin sau. Đây là đánh đổi cần ghi rõ.
- Khác hội thoại: song song, có giới hạn tổng (ví dụ 4).

**Văn bản theo sau upload:** upload chạy ở lane media. Chỉ bước *gửi metadata* sau `file_done` / `video_done` mới vào hàng FIFO của hội thoại. Văn bản không chờ upload; thứ tự hiển thị theo thời điểm từng tin được *server chấp nhận*. Cách này khớp vòng đời bất đồng bộ của PC (E2: `_processFileAsyncDoneEvent`), còn việc PC có tuần tự theo hội thoại hay không vẫn **AMBIGUOUS**.

| Kiểm chứng được ở client (mock transport) | Chỉ kiểm chứng được bằng tài khoản thật (E1) |
|---|---|
| Thứ tự khởi phát; tối đa 1 request đang chạy mỗi hội thoại; văn bản không bị upload chặn; không tự gửi lại khi kết quả mơ hồ | Thứ tự `msgId`/`ts` server trả về; thứ tự echo trên thiết bị khác; hành vi khi hai request chồng nhau |

## 7. Các điểm còn lại

| Điểm của Codex | Đánh dấu |
|---|---|
| Khi bộ đệm socket đầy, reader chờ 5 giây và **không chạy nhánh ping** | **Đồng thuận.** `forward_event` nằm trong nhánh đọc của `select!`, nên ping không được phục vụ trong lúc chờ. Bổ sung so với vòng 1. |
| Receipt chờ SQLite ở [worker.rs:5197](../../src/gui/worker.rs#L5197) | **Đồng thuận** (chưa đọc kỹ). Cùng nhóm với lỗi tranh khóa ghi ở mục 2.2. |
| Không đưa payload chưa qua bộ lọc tin đã xoá hoặc chính sách hiển thị thẳng lên UI | **Đồng thuận.** Hiển thị sớm phải đi sau `is_deleted` và hidden-view; chỉ phần tra tên/profile được hoãn. |
| Không sao chép nhánh nhảy cursor `__checkReset` | **Đồng thuận.** |

## 8. Thứ tự làm đề xuất

1. **Sửa lỗi bỏ lệnh sau đăng nhập** (mục 1). Đây là regression của patch đang dở và phải có trước khi commit patch.
2. P0: đo theo mục 2.3.
3. Bỏ `persist_threads_async` khỏi `apply_outgoing_state`; gộp các lần còn lại (mục 2.2).
4. Tách `drain` khỏi đường xử lý chính, giữ `enqueue` làm điều kiện cursor (mục 4).
5. Hoãn tra tên/profile (vòng 1, đồng thuận).
6. Lane outgoing: FIFO, mỗi hội thoại tối đa 1 request đang chạy; khi không khớp phiên thì phát event kết thúc (mục 1 và 6).
7. Chỉ cân nhắc watermark hoặc lane ưu tiên khi số đo sau bước 3-5 cho thấy cần.
