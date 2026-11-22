# Phản biện: "Kiến trúc realtime và đồng bộ lịch sử của ZaloLite" (bản Codex 2026-11-21)

Ngày: 2026-11-21 · Người phản biện: Claude
Đối tượng: `docs/research/2026-11-21-realtime-vs-history-architecture-review.md` và các thay đổi **chưa commit** trong `src/gui/worker.rs`, `src/outgoing.rs`, `src/service_resolver.rs`.

Nguồn bằng chứng:

- Mã fork ở working tree hiện tại (HEAD `8e235f7` cộng diff chưa commit).
- Bundle Zalo PC 26.11.12, giải nén tĩnh từ `C:\Users\bqqhu\AppData\Local\Programs\Zalo\Zalo-26.11.12\resources\app.asar`. Chỉ lấy `pc-dist/*.js` vào thư mục tạm; không đọc DB, cookie hay dữ liệu tài khoản. Không cần đến công cụ `rea`: một trình đọc ASAR tối giản bằng PowerShell là đủ, nên tôi không tải thêm mã bên thứ ba nào.
- Nhãn bằng chứng: E1 là capture live, E2 là đọc tĩnh bundle, E3 là suy luận. Không có kết luận nào trong bản này đạt mức E1.

---

## 0. Tóm tắt

| # | Kết luận của Codex | Đánh giá |
|---|---|---|
| 1 | Vòng nhận WebSocket còn chờ SQLite, resolve tên và reaction trước khi xử lý frame kế tiếp | **Đúng, nhưng nêu sai chỗ.** Socket reader ở `socket.rs` *không* bị chặn. Cái bị chặn là consumer trong `worker.rs`. Mức nghiêm trọng còn cao hơn Codex mô tả (mục 1.2). |
| 2 | Lỗi `QueueWait` khi gửi đến từ việc biến thời gian chờ queue thành lỗi | **Đúng về triệu chứng, chưa tìm ra nguyên nhân gốc.** Codex chưa chỉ ra lệnh nào chặn command loop. Ứng viên số một là `RunAutomation` chạy mỗi 2 giây, cộng `busy_timeout` 5 giây của SQLite (mục 2.1). |
| 3 | Bản sửa outgoing là "đúng hướng" | **Đúng một phần.** Nó tạo một lane riêng, nhưng chép lại một race có sẵn và vi phạm ngay hai invariant do tài liệu tự đặt ra (mục 2.2). |
| 4 | Đề xuất "ordered durable writer" (một writer tuần tự) | **Lệch so với Zalo PC.** PC dùng mô hình *low-watermark* với nhiều batch xử lý song song, không dùng writer tuần tự (mục 3.1, E2). |
| 5 | Reaction phải được lưu xong trước khi commit cursor | **Lệch so với Zalo PC.** PC xử lý reaction trễ (`setTimeout 0`) và không tính reaction vào bộ đếm batch (mục 3.2, E2). |
| 6 | Có thể cần sửa protocol | **Không cần.** Mọi vấn đề đã tìm thấy đều nằm ở scheduling và ownership nội bộ. |

---

## 1. Đường nhận realtime

### 1.1. Codex mô tả sai tầng bị chặn

`src/socket.rs:825-833` đã tách socket reader khỏi consumer bằng một hàng đợi `FORWARD_QUEUE = 512` (`socket.rs:436`) cộng channel downstream 32 (`socket.rs:670`). Nhờ vậy reader vẫn gửi và nhận ping khi consumer chậm. Câu "WebSocket reader không được chờ SQLite" (Invariant 1) vì thế **đã đúng** ở tầng transport.

Thứ bị tuần tự hoá là **consumer** `tokio::spawn` tại `src/gui/worker.rs:1487-1630`. Hai tầng này hỏng theo hai cách khác nhau:

- Consumer chậm gây head-of-line latency cho các frame phía sau, nhưng không mất kết nối.
- Consumer bị treo quá lâu làm tràn hàng đợi 512 và quá 5 giây (`socket.rs:448-459`). Khi đó transport `bail!` và **đóng epoch, phải reconnect**. Một luồng tin dồn dập trong lúc SQLite bị khoá có thể kéo theo reconnect liên hoàn. Tài liệu Codex chưa nêu hệ quả này.
- Khi `forward.capacity() < 128` (`socket.rs:875`), reader **ngừng pull catch-up**, nên consumer chậm còn làm chậm cả việc kéo bù tin.

### 1.2. Bằng chứng theo dòng (trả lời câu hỏi 1)

Mỗi frame đi qua các bước tuần tự sau trong `worker.rs`:

| Dòng | Việc làm | Độ trễ tệ nhất |
|---|---|---|
| 1493 | `s_ws.lock().await.clone()`: lấy mutex session **mỗi frame** | Thường ngắn. Các chỗ giữ khóa lâu mà tôi kiểm tra đều đã clone rồi thả. |
| 1530-1534 | `extract_voip_controls` → `cb.handle_control_event` | Nhanh, và **chạy trước** các bước chậm trong cùng frame |
| 1536 | `deliver_presence(...).await` | Chưa đo |
| 1537-1545 | Reaction: `spawn_blocking`, **mở `MessageStore` mới**, enqueue rồi drain, rồi `.await` | Tới 5 giây vì `busy_timeout` (`store.rs:218`) |
| 1600-1604 | `resolve_thread_names(...).await`: **HTTP** `getminiprofiles` và/hoặc group metadata | Tới 35 giây mỗi request (`session.rs:539`), có thể nhiều request vì mỗi chunk 50 ID |
| 1605 | `persist_received(...).await`: mở store mới, ghi | Tới 5 giây busy, cộng thời gian ghi |
| 1606-1609 | `publish_live` → UI | **Chỉ chạy sau tất cả các bước trên** |
| 1612 | `commit_socket(...).await`: thêm một `spawn_blocking` SQLite nữa | Tới 5 giây |

Hai phát hiện mới mà Codex chưa nêu:

1. **`resolve_thread_names` gọi HTTP cho gần như mọi tin 1-1 đến.** Bộ lọc ở `src/chat.rs:5410-5421` là `official_account.is_none() || ...`. `ws_threads` được dựng mới từ payload (`worker.rs:1575-1597`), nên `official_account` hầu như luôn là `None`, tức điều kiện đúng với mọi thread 1-1. Hệ quả là **mỗi tin đến phải chờ một HTTP round-trip rồi bubble mới hiện**, và `get_mini_profiles` (`chat.rs:5304`) không có cache. Đây có lẽ là đóng góp lớn nhất vào độ trễ phía nhận khi mạng chậm. Ghi chú: nhận định này dựa trên việc đọc mã; chưa đo thực tế.
2. **Cuộc gọi không chờ SQLite *trong cùng frame*, nhưng vẫn chờ các frame đứng trước.** Câu "cuộc gọi đến phải chờ reaction drain" của Codex chỉ đúng một nửa. Frame VoIP được xử lý ngay khi tới lượt (`1530`), nhưng nếu frame chat trước đó đang chờ HTTP 35 giây thì frame VoIP 601/603 cũng phải đợi. Fork có đường dự phòng `lp_control` (`worker.rs:1634-1665`), nhưng đường này chạy song song với WebSocket chứ không thay thế. Nhận định này là E3 vì chưa đo.

### 1.3. Đối chiếu Zalo PC (E2): ingress không chờ DB, nhưng vẫn xử lý tuần tự có gom batch

Trong `compact-app-pc.0cd2e653e67efaa4bf4b.js`, khoảng offset 14.553.000, hàng đợi sự kiện `s` hoạt động như sau:

- Frame chat (`t:1`) và control (`t:2`) được `push` vào **cùng một queue** (`_` và callback `connectCtr`).
- Hàm `m()` lấy phần tử đầu, rồi **gộp mọi phần tử liền kề cùng loại** thành một batch (`mergeChatBatch` / `mergeCtrlBatch`), gọi `h()` (chat) hoặc `A()` (ctrl).
- Lượt kế tiếp được hẹn bằng `setTimeout(m, 100)` ngay sau khi `h()` *bắt đầu*. Ghi DB chạy bất đồng bộ (`Promise.all(f)`) và **không được await trước batch sau**.

Hệ quả cho thiết kế: PC vẫn xử lý **tuần tự theo thứ tự frame** ở tầng dispatch, nhưng tách *dispatch* khỏi *durability*. Đề xuất "realtime plane / durability plane" của Codex khớp với hướng này. Tuy vậy, chia ra 5 queue ưu tiên riêng (call / receipt / chat / reaction / durable) thì **phức tạp hơn PC**. PC chỉ dùng một queue có gom batch, cộng `chatHandler` và `ctrlHandler` theo dõi cursor riêng (`saveChatData` / `saveCtrlData`, offset ~10.667.000).

**Khuyến nghị:** giữ **một** consumer tuần tự như hiện nay, nhưng (a) bỏ mọi `.await` mạng ra khỏi consumer, (b) chuyển ghi DB sang mô hình watermark ở mục 3.1, (c) gom các frame chat liền kề thành một transaction. Chưa nên tạo 5 lane ưu tiên khi chưa có số đo chứng minh cần thiết.

---

## 2. Đường gửi

### 2.1. Nguyên nhân gốc của `QueueWait` chưa được xác định

Ở HEAD, outgoing đã được `tokio::spawn` ngay sau khi lấy khỏi queue. Bằng chứng là `git show HEAD:src/gui/worker.rs` khoảng dòng 1714-1755, và đường cũ này vẫn còn ở working tree quanh dòng 1760-1800. Vậy thời gian chờ 5 giây trở lên phải đến từ **một handler khác `.await` ngay trong command loop**, khiến lệnh gửi không được lấy ra.

Ứng viên mạnh nhất (E3, dựa trên mã):

- `worker.rs:1748`: một ticker gửi `RunAutomation` và `PruneExpired` vào command queue **mỗi 2 giây**.
- `worker.rs:1965-1976`: handler `RunAutomation` **await trực tiếp trong loop** một `spawn_blocking`. Hàm này mở `MessageStore`, chạy `tools::recover` rồi `tools::claim`, mà `claim` là một thao tác ghi.
- `store.rs:218`: `busy_timeout = 5s`. Khi `persist_received`, reaction drain hoặc history sync đang giữ khoá ghi WAL, `claim` phải chờ tới **5 giây**. Con số này trùng `QUEUE_BUDGET = 5s` và trùng triệu chứng người dùng báo ("nhanh nhất khoảng 5 giây").
- Thêm vào đó, mỗi lần gọi `MessageStore::open` (`store.rs:206-226`) đều chạy lại `PRAGMA` và `CREATE TABLE IF NOT EXISTS`, nên mỗi thao tác phải mở kết nối mới.

**Việc cần làm trước P1:** đo thời gian từng handler của command loop theo loại `WorkerCmd`, không ghi payload. Nếu `RunAutomation` hoặc `PruneExpired` vượt 100 ms, chuyển chúng sang `tokio::spawn`, vì đã có `automation_running` làm guard. Bỏ hẳn `QueueWait` mà không sửa điểm này thì chỉ **giấu triệu chứng**: mọi lệnh khác như load draft, search hay settings vẫn bị trễ 5 giây.

### 2.2. Lỗi trong bản sửa outgoing chưa commit

| Vấn đề | Vị trí | Mức độ |
|---|---|---|
| **Không giữ thứ tự trong cùng một hội thoại.** Mỗi tin là một `tokio::spawn` độc lập, nên tin 1-2-3 gửi liền nhau có thể tới server theo thứ tự khác. Mục 6.3 của Codex yêu cầu "cùng conversation giữ ordering" nhưng mã không làm điều đó. Lỗi đã có từ HEAD, bản sửa chép lại y nguyên. | `worker.rs:1346-1352`, `5598` | Cao. Thứ tự server phụ thuộc thứ tự request; PC có tuần tự hoá theo hội thoại hay không hiện là **AMBIGUOUS**, cần E2/E1. |
| **Race giữa hai lần persist.** Lần ghi `Pending` chạy trong một `tokio::spawn` riêng (`5627`), song song với lần ghi trạng thái cuối (`5644`). Nếu lần ghi pending xong *sau*, SQLite có thể bị đè về `Pending`, hoặc nếu `msg_id` đã đổi sang ID server thì để lại một dòng pending "ma" khi khởi động lại. Lỗi cũng có từ HEAD. | `worker.rs:5623-5628` vs `5644` | Trung bình (PLAUSIBLE, cần test) |
| **Lặp mã.** `spawn_outgoing_send` là bản sao của đường xử lý trong command loop. Đường cũ vẫn chạy cho `RetryOutgoing` gửi thẳng qua `cmd_tx`, ví dụ khi tiếp tục upload (`worker.rs:2527`). Hai bản sẽ lệch nhau theo thời gian. | `worker.rs:~1760` vs `5598` | Thấp. Nên cho đường cũ gọi `spawn_outgoing_send`. |
| Channel outgoing là `unbounded_channel` | `worker.rs:1291` | Thấp. Trái Invariant 9 của chính tài liệu; chấp nhận được vì do người dùng kích hoạt, nhưng nên ghi rõ là ngoại lệ. |
| Test vẫn khẳng định `QueueWait` có `BudgetExceeded` | `outgoing.rs` test `delayed_command_queue_is_timed_but_does_not_expire_send_budget` | Thấp. Nhãn `BudgetExceeded` cho một metric thuần chẩn đoán dễ gây hiểu nhầm khi đọc log. |

**Khuyến nghị:** dùng một outgoing actor có `HashMap<thread_id, VecDeque<intent>>`: tuần tự trong mỗi hội thoại, song song giữa các hội thoại. Gộp hai lần persist thành một tác vụ tuần tự (pending → send → final) trong cùng actor đó.

---

## 3. Cursor và durability (trả lời câu hỏi 2, 3, 6)

### 3.1. Zalo PC dùng low-watermark, không dùng một writer tuần tự (E2)

Trong `compact-app-pc...js`, khoảng offset 1.671.000-1.674.000 và 14.572.000:

- `addLastActionIds` → `__markWaitId(saveActionId)`: mỗi batch nhận vào được thêm vào tập `__waitId`.
- `addChatCounter(saveActionId, f.length)`: một batch cần N lần ghi tin thành công.
- `_successWriteData(e){ e && saveChatData(e) }`: được gọi **sau khi ghi DB tin nhắn thành công**. Mỗi lần gọi giảm bộ đếm đi 1.
- `saveData(t)`: chỉ khi bộ đếm về 0 *và* `t == min(__waitId)` thì cursor mới được lưu (`_saveData`). Nếu batch xong trước một batch nhỏ hơn đang chờ thì cursor đứng yên.
- `__checkReset`: nếu `__maxDoneId - min(waitId) > polling_conf.reset_max_diff` (mặc định `1e3`, offset ~8.073.600) thì PC **bỏ cuộc và nhảy cursor lên `__maxDoneId`**, chấp nhận khả năng mất tin để không kẹt vĩnh viễn.
- `getLastActionIdsForSocket` trả `{lastId, preIds: ...slice(0,10)}`, khớp với `re-notes/21`.

Câu hỏi 3 của Codex hỏi nên dùng writer theo account, theo queue hay toàn worker. PC không chọn phương án nào trong số đó: các lần ghi chạy song song, còn **cursor là low-watermark theo từng handler** (chat và ctrl tách riêng).

Áp dụng cho fork:

- Không cần một writer tuần tự cứng nhắc. Cần một `WatermarkTracker` cho mỗi `(account, handler)` gồm: `pending: BTreeMap<batch_seq, remaining>`, và commit cursor = batch nhỏ nhất đã xong liên tục.
- Có thể giới hạn bộ nhớ: giới hạn số batch đang chờ (ví dụ 64). Khi đầy thì consumer ngừng dispatch, và áp lực ngược sẽ lan về `socket.rs`, nơi đã có watermark 128 để ngừng pull. Vì vậy **không cần queue không giới hạn**.
- Fork **chưa nên** chép `reset_max_diff` (nhảy cursor và chấp nhận mất tin). Đó là một đánh đổi sản phẩm cần quyết định riêng. Fork hiện giữ cursor để replay, an toàn hơn; chỉ cần ghi rõ đây là **"Fork deviation"**.

### 3.2. Reaction: fork đang chặt hơn PC (E2)

- PC, tại offset ~14.554.000: `r.length>0 && setTimeout(()=>processReactionMessages(r,t,!0),0)`. Reaction được xử lý **sau** batch tin và **không nằm trong mảng `f`** dùng cho `addChatCounter`.
- Fork, tại `worker.rs:1539-1545` và `1612`: `commit_socket(..., stored && reactions_stored)`. Chỉ cần lưu reaction lỗi là **cả page tin nhắn bị replay**.

Đề xuất: chuyển reaction ra khỏi đường xử lý chính, giống PC, và chỉ gate cursor theo việc lưu **tin nhắn**. Nếu muốn giữ yêu cầu không mất reaction, hãy cho reaction một bộ đếm watermark riêng thay vì chặn tin. Hai cách đều phải ghi nhãn rõ ràng: cách thứ nhất là "Parity with Zalo PC (E2, bundle 26.11.12)", cách thứ hai là "Fork deviation".

### 3.3. Duplicate mutation (câu hỏi 6)

- Cursor: bản sửa chưa commit **không** đụng vào `commit_socket`, nên không vi phạm "cursor chỉ commit sau durable write".
- Mutation: bỏ `QueueWait` thì giảm lỗi giả `NotSent`, nên **giảm** nguy cơ người dùng bấm gửi lại và tạo tin trùng. `Active::start` vẫn chặn hai lần gửi cùng `client_message_id`. Rủi ro còn lại là race persist ở mục 2.2. Nếu một dòng pending "ma" được nạp lại kèm `outgoing_job`, nó có thể bị hiểu là một lần thử lại. Cần một test: pending persist chậm hơn final persist thì trạng thái cuối trong SQLite phải là `Sent`.

---

## 4. Gắn lại nhãn bằng chứng (câu hỏi 5)

| Phát biểu trong tài liệu Codex | Nhãn đúng |
|---|---|
| PC có `zpw_ws`, chat 501/521/510/511/513-518/570, control 601-604 | E2 (cũng khớp `socket.rs`) |
| Cursor tách "received" và "processed", `preIds ≤ 10` | **E2, đã kiểm tra lại**. Còn thiếu cơ chế watermark và `reset_max_diff` (mục 3.1). |
| `file_done` / `video_done` bất đồng bộ | **E2, đã kiểm tra lại** (`_processFileAsyncDoneEvent`, `_processVideoAsyncDoneEvent`, offset ~14.582.000) |
| `sync-v2-worker` / `sync-v2-sub-worker` riêng | E2 (có file `sync-v2-sub-worker.*.js` trong bundle 26.11.12) |
| `max_number_of_concurrent_sync_items` mặc định 4 | **E2 chưa xác minh lại**. Tôi chỉ tìm thấy *tên* khoá trong `lazy/main-startup.*.js`, chưa thấy giá trị mặc định. |
| Có `batch_timeout`, retry | E2 một phần: thấy metric `wk_batch_timeout_count`; chưa đọc logic retry |
| "VoIP control xử lý qua call state/native bridge, tách khỏi history sync" | E2 về cấu trúc (`ctrlHandler` riêng). Còn câu "không được chờ…" là **yêu cầu của fork (E3)**: PC đưa ctrl vào **cùng queue `s`** với chat. |
| "Realtime plane / durability plane" là cách PC làm | E2 một phần: PC tách dispatch khỏi durability, nhưng **không** chia 5 lane ưu tiên. Các lane là đề xuất của fork (E3). |
| PC dùng `dal-worker`, `zd-worker`, `opfs-worker` cho dữ liệu | E2 chỉ ở mức tên process (offset ~7.117.000). Chưa đọc vai trò. |
| PC không resolve profile trước khi hiển thị tin đến | **Chưa có bằng chứng**, coi là AMBIGUOUS. Dù vậy, việc fork chặn trên HTTP là một deviation chứ không phải parity, nên có thể sửa mà không cần bằng chứng từ PC. |

---

## 5. Có cần sửa protocol không? (câu hỏi 8)

**Không.** Mọi vấn đề trên đều là scheduling và ownership nội bộ: consumer await mạng, SQLite `busy_timeout` cộng việc mở kết nối cho mỗi thao tác, handler await trong command loop, thứ tự outgoing, và cách gate cursor theo reaction. Không có đề xuất nào cần đổi command, param, cipher hay frame ID.

---

## 6. Kế hoạch đề xuất (sửa lại mục 8 của Codex)

**P0: đo, không ghi nội dung.** Đây cũng là câu trả lời cho câu hỏi 7.

- Gắn `Instant` cho từng bước 1493 → 1612 trong consumer WebSocket, và cho mỗi handler của command loop theo **tên biến thể `WorkerCmd`**. Ghi vào `diagnostics/*.jsonl` chỉ các trường `{phase, cmd_kind, elapsed_ms, queue_len}`; không ghi thread ID, nội dung, URL hay cookie.
- Đo p50/p95 cho hai khoảng: Enter → `OutgoingState(Pending)` được *render*, và socket frame được *nhận* → `LiveThreadsReady` được *render*. Dùng chung một `Instant` truyền qua event; đo trên máy ARM64 thật với build release.

**P1: các sửa nhỏ, rủi ro thấp, làm trước khi tái cấu trúc lớn.**

1. Đưa `resolve_thread_names` ra khỏi consumer: hiển thị bằng ID hoặc tên cache trước, resolve trong một task nền có giới hạn và cache theo ID, rồi gửi `ThreadsUpdated` sau.
2. Đưa reaction ra khỏi đường xử lý chính và khỏi điều kiện gate cursor (mục 3.2).
3. Chuyển `RunAutomation` và `PruneExpired` ra khỏi command loop.
4. Giữ một kết nối SQLite cho mỗi account trong worker thay vì `open` cho mỗi thao tác, hoặc ít nhất tách bước migrate khỏi `open`. Cân nhắc giảm `busy_timeout` cho các đường không quan trọng.
5. Outgoing: actor tuần tự theo hội thoại, gộp hai lần persist.

**P2: durability theo watermark** (mục 3.1), với giới hạn số batch đang chờ và áp lực ngược về `socket.rs`. Chỉ làm khi số đo P0 sau P1 vẫn cho thấy SQLite trên đường xử lý chính là nút thắt.

**P3: chia lane ưu tiên.** Chỉ làm nếu P0 chứng minh call control vẫn bị trễ sau P1 và P2.

---

## 7. Tiêu chí nghiệm thu cần thêm

- **Burst:** 600 frame chat trong khi SQLite bị giữ khoá ghi 6 giây thì **không** được reconnect (không có `event queue overflow`), và cursor không nhảy qua page chưa được lưu.
- **Thứ tự:** gửi liên tiếp 20 tin vào một hội thoại, với transport giả lập độ trễ ngẫu nhiên, thì thứ tự request tới server phải đúng 1 đến 20.
- **Race persist:** lần ghi pending bị trì hoãn sau lần ghi final thì trạng thái cuối trong SQLite phải là `Sent` và không có dòng thừa.
- **Reaction:** lưu reaction lỗi thì cursor tin nhắn vẫn tiến (nếu chọn parity với PC), hoặc reaction được replay độc lập (nếu chọn deviation).
- **Name resolve:** tin 1-1 đến khi endpoint profile treo 30 giây thì bubble vẫn hiện dưới 300 ms (fixture).

## 8. Giới hạn của bản phản biện

- Chưa build hay chạy test; ghi chú của Codex cho biết `ring` thiếu `clang.exe`. Mọi nhận định về độ trễ là E3 từ đọc mã, cần số đo P0.
- Offset trong bundle tính trên `compact-app-pc.0cd2e653e67efaa4bf4b.js` (Zalo PC 26.11.12) và sẽ thay đổi theo bản phát hành.
- Chưa có capture live (E1) nào.
