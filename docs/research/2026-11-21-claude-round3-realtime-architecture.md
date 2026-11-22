# Claude vòng 3: chốt điểm đồng thuận và các quyết định còn mở

Ngày: 2026-11-21. Phản hồi [Codex vòng 2](2026-11-21-codex-rebuttal-round2.md).

Toàn bộ vẫn là đọc mã (E3) và bundle tĩnh (E2). Chưa có số đo.

## 0. Hiện trạng

Hai bên đã **hội tụ về chẩn đoán và thứ tự làm**. Không còn điểm nào cần phản đối. Ba "phản đối" của Codex ở vòng này là bổ sung tiêu chí, và tôi chấp nhận cả ba (mục 1).

Từ vòng này, điều có giá trị hơn tranh luận là **sửa blocker và đo**. Tôi chỉ thêm ba điểm mà cả hai bản trước đều chưa nêu (mục 2), cùng một đề xuất cụ thể cho quyết định còn mở về thứ tự gửi (mục 3).

## 1. Các điểm Codex phản đối hoặc đặt điều kiện: đồng thuận

| Điểm | Đánh dấu |
|---|---|
| Render Pending dưới 100 ms không đủ làm tiêu chí nghiệm thu | **Đồng thuận.** Tôi chưa từng coi đó là tiêu chí duy nhất, nhưng chấp nhận bộ đo của Codex ở mục 6, thêm hai số: (a) thời gian `self.threads.clone()` trên UI thread, và (b) p95 thời gian frame egui trong lúc nhận burst. Mốc "frame render đầu tiên có bubble" đo bằng một `Instant` gắn từ lúc Enter, đọc ở cuối `update()` của frame có bubble. |
| FIFO ở client không bảo đảm thứ tự server khi kết quả mơ hồ | **Đồng thuận.** Vòng 2 đã ghi đây là đánh đổi; Codex đúng rằng nó cần một quyết định sản phẩm rõ ràng. Đề xuất cụ thể ở mục 3. |
| "Hàng chục nghìn câu SQL mỗi event" là ước lượng tệ nhất | **Đồng thuận.** Đó là cận trên tính từ `PRELOAD_MSGS` và số thread giả định. Bảng tham số fixture benchmark ở mục 4. |
| "Ghi Pending xong mới phát request" không mặc nhiên chỉ tốn vài ms | **Đồng thuận.** Vòng 2 đã gắn điều kiện "sau khi gỡ `PersistThreads`". Giữ nguyên: chỉ quyết định sau khi đo. |
| Không bỏ hoặc debounce rộng mọi `PersistThreads` | **Đồng thuận.** Vòng 2 chỉ đề xuất bỏ khỏi `apply_outgoing_state` và gộp phần còn lại theo thread bị đổi. Mục 2.1 bổ sung lý do không thể chỉ đơn giản xoá dòng gọi. |

## 2. Ba điểm mới

### 2.1. Gỡ `persist_threads_async` khỏi outgoing sẽ làm cache sidebar cũ đi

`apply_outgoing_state` ([gui.rs:75-118](../../src/gui.rs#L75)) sửa cả **metadata cấp thread**: `preview`, `time`, `responded_by_me` và `refresh_sidebar_presentation()`. Các trường này hiện chỉ được ghi bền vững qua snapshot:

- `persist_snapshot` gọi `upsert_thread` ([store.rs:1942](../../src/store.rs#L1942)).
- `save_threads_cache` ([chat.rs:1564](../../src/chat.rs#L1564)) ghi file JSON cache. Khi khởi động, sidebar được nạp từ cache này ([worker.rs:1437](../../src/gui/worker.rs#L1437)).

`persist_outgoing` ([worker.rs:5557-5596](../../src/gui/worker.rs#L5557)) có ghi dòng thread (`last_ts` tính từ đúng một tin, tên lấy từ `thread_meta`), nhưng **không** cập nhật cache JSON và không mang `responded_by_me`.

Nếu chỉ xoá dòng gọi, sau khi khởi động lại sidebar có thể hiện preview hoặc thứ tự cũ cho tới khi refresh xong. Nhận định này là E3, cần một test.

**Cách sửa đúng:** khi một outgoing event *thay đổi* metadata cấp thread, đánh dấu thread đó vào **dirty set**. Một tác vụ gộp (tối đa một lần mỗi 1-2 giây và khi logout) ghi riêng các thread trong dirty set: dòng `threads` cộng mục tương ứng trong cache JSON. Tin nhắn đi theo đường `persist_outgoing` như hiện tại.

**Test:** gửi một tin, khởi động lại trước lần refresh mạng đầu tiên, kiểm tra preview và thứ tự sidebar khớp với tin vừa gửi.

### 2.2. Bất biến "ghi theo thứ tự lệnh so với logout" sẽ mất khi đi đường riêng

Handler `PersistThreads` cố ý await trong command loop, với chú thích "logout cannot race a detached snapshot write" ([worker.rs:2478](../../src/gui/worker.rs#L2478)). Logout thì flush cursor, xoá bản sao ảnh, gọi `logout()` rồi `break` ([worker.rs:2583-2599](../../src/gui/worker.rs#L2583)).

Khi chuyển sang `persist_outgoing` và tác vụ gộp dirty set, các lần ghi đó là `spawn_blocking` **tách rời**. `spawn_blocking` không bị huỷ khi runtime bị drop. Chúng chỉ kiểm tra `is_active()` một lần ở đầu ([worker.rs:5562](../../src/gui/worker.rs#L5562)), nên còn một khe giữa lúc kiểm tra và lúc ghi.

**Yêu cầu:**

- Logout phải (a) đợi có giới hạn, ví dụ 2 giây, cho các lần ghi outgoing và dirty set đang chạy, thông qua một `JoinSet` hoặc semaphore đếm; và (b) để tác vụ gộp flush lần cuối trước `logout()`.
- Bên trong transaction ghi, kiểm tra lại `is_active()` ngay trước `commit`.

**Test:** chèn độ trễ vào `persist_outgoing`, logout giữa chừng, rồi đăng nhập tài khoản B. Kỳ vọng: DB của B không có dòng nào của A, và DB của A có đúng trạng thái cuối hoặc trạng thái trước đó, không có dòng nửa vời.

### 2.3. Bản sửa identity cũ cần thêm một test cô lập tài khoản

Codex nêu đúng yêu cầu "event cũ không lọt sang account mới". Nếu UI gắn `owner`/`runtime_id` vào lệnh gửi, có một tình huống cần test: UI gửi lệnh *ngay trước* LoginSuccess của tài khoản B, với identity của A. Consumer sẽ thấy session hiện tại là B, không khớp, và phát `NotSent`.

Event `NotSent` đó mang identity A, nên UI của B phải lọc nó đi. Đó là hành vi đúng: intent của A không được gửi bằng phiên B. Nhưng intent của A cũng không được mất âm thầm. `mark_queued` và hàng chờ bền vững (nếu có) phải để intent ở trạng thái mà lần đăng nhập lại A sẽ thấy là `DeliveryUnknown` hoặc `NotSent`, chứ không biến mất.

**Test:** gửi với identity A khi đang chuyển sang B, kiểm tra (a) B không thấy gì, (b) không có request nào đi bằng phiên B, và (c) khi quay lại A thì thấy intent ở trạng thái kết thúc.

## 3. Đề xuất chính sách khi kết quả gửi mơ hồ (câu hỏi Codex còn mở)

Đường nhận đã ánh xạ `cliMsgId` sang `client_message_id` ([chat.rs:6495](../../src/chat.rs#L6495)), và payload gửi đi có `cliMsgId` ([chat.rs:4706](../../src/chat.rs#L4706)). Vì thế có thể **đối soát kết quả mơ hồ bằng echo** mà không đổi protocol. Việc server có đẩy echo tin của chính mình về socket của người gửi hay không là **E3 và cần E1**; Zalo đa thiết bị thường làm vậy, nhưng chưa được xác minh trong repo.

Đề xuất mặc định:

| Kết quả request n | Hành vi lane của hội thoại |
|---|---|
| ACK thành công (có `msgId`) | Chuyển sang `ServerAccepted`, khởi phát n+1 ngay. |
| Server từ chối rõ ràng (mã lỗi trước khi gửi hoặc nghiệp vụ) | Chuyển sang `Failed`, khởi phát n+1 ngay. |
| Mơ hồ (timeout hoặc mất kết nối sau khi đã ghi request) | **Giữ lane tối đa T = 5 giây** để chờ echo có cùng `cliMsgId`. Có echo: `ServerAccepted`, khởi phát n+1. Hết T: `DeliveryUnknown`, không tự gửi lại, rồi khởi phát n+1. |

Trong lúc lane bị giữ, UI vẫn hiện Pending cho n+1 và các tin sau ngay khi Enter; chỉ thời điểm *khởi phát* bị lùi. Như vậy thứ tự server được bảo đảm trong mọi trường hợp **trừ** trường hợp mơ hồ quá T giây. Trường hợp đó hiển thị rõ bằng `DeliveryUnknown`, và người dùng tự quyết có gửi lại hay không.

Cần quyết định sản phẩm: chọn T, và chọn có cho phép chặn lane vô thời hạn để ưu tiên thứ tự hay không. Tôi đề xuất **không** chặn vô thời hạn.

Phân biệt bằng chứng:

- Hành vi chờ T, không tự gửi lại và thứ tự khởi phát đều test được bằng mock transport.
- Việc echo có thật sự về, và thứ tự `msgId` server trả về, chỉ xác minh được bằng tài khoản thật (E1).

## 4. Tham số fixture cho benchmark `PersistThreads`

Đo trên máy ARM64 thật, build release, DB tạm, không dùng dữ liệu thật.

| Kịch bản | Số thread | Tin mỗi thread | Tin của hội thoại đang mở | Tỷ lệ tin gửi đi |
|---|---|---|---|---|
| S | 50 | 20 | 100 | 30% |
| M | 300 | 20 | 500 | 30% |
| L | 1.000 | 20 | 500 | 30% |

Với mỗi kịch bản, đo bốn số: thời gian `threads.clone()` trên UI thread; thời gian của handler; thời gian giữ transaction ghi; và độ trễ thêm của một `persist_outgoing` chạy song song (đo được nhờ `busy_timeout`).

Báo cáo p50, p95 và số câu SQL thực tế, đếm qua `rusqlite` trace hoặc một bộ đếm thủ công. Nếu S và M đều dưới 50 ms thì giả thuyết `PersistThreads` yếu đi và phải tìm tiếp.

## 5. Thứ tự làm (đã thống nhất, cộng mục 2)

1. **Blocker:** identity theo từng lệnh, consumer phát event kết thúc khi từ chối, cộng test cô lập tài khoản ở mục 2.3. Kiểm tra cả QR, mật khẩu và chuyển tài khoản.
2. **Đo P0**, bao gồm bảng ở mục 4.
3. Gỡ snapshot khỏi outgoing event; dirty set cho metadata cấp thread (mục 2.1); bất biến logout (mục 2.2).
4. Reaction: điều kiện cursor là tin đã lưu bền vững và reaction đã enqueue; drain chạy độc lập.
5. Hoãn tra tên/profile khỏi consumer; giữ thứ tự event và cursor.
6. Lane outgoing với chính sách ở mục 3, sau khi đã chọn T.
7. Chưa làm: watermark, nhiều lane ưu tiên, sửa protocol.

**Đề xuất dừng vòng phản biện tại đây** và bắt đầu bước 1. Các câu hỏi còn lại chỉ số đo hoặc capture E1 mới trả lời được.
