# Kế hoạch hành động: realtime, gửi tin và các lỗi UI/UX

Ngày: 2026-11-21

Trạng thái: **P1–P3 đã có mã, qua kiểm thử và được hai audit độc lập chấp thuận; P4/P5 đã bổ sung mã và checklist, native acceptance còn cần chạy trên build test tiếp theo.**
Phạm vi: giảm trễ và lỗi gửi/nhận, bảo vệ ranh giới tài khoản, sau đó xử lý backlog UI/UX đã báo.

## Quyết định

Ưu tiên sửa đúng luồng và đo trước khi tái cấu trúc rộng. Giữ giao thức Zalo hiện tại; không thêm lane, watermark hay thứ tự server giả định nếu chưa có bằng chứng. Bubble `Pending` phải xuất hiện ngay sau khi UI chấp nhận intent; kết quả gửi phải phản ánh ACK/echo hoặc trạng thái chưa xác định, không biến chờ local queue thành lỗi gửi.

Khi logout/đổi tài khoản, chọn **barrier ghi nghiêm ngặt**: generation mới và session mới không được hoạt động cho đến khi mọi ghi của generation cũ đã commit hoặc abort. Hết thời gian chờ chỉ được báo “đang chờ đóng phiên”; không được vượt barrier rồi để writer cũ commit sau. Nếu triển khai barrier bằng lock phức tạp hoặc gây deadlock, chuyển sang coordinator/writer sở hữu tuần tự qua message passing, không nới invariant.

Giữ `Queue cursor` chỉ tiến sau khi dữ liệu tương ứng đã được lưu bền vững theo ADR-0026. Reaction phải được enqueue bền vững trước khi commit cursor; việc drain/hiển thị reaction không được giữ WebSocket reader lại. Identity của mỗi intent và event gắn với owner/runtime nguồn; lệnh cũ không bao giờ được gửi bằng session mới. Không hứa khôi phục intent chưa ghi bền vững.

Đối chiếu Zalo PC/Web hiện có là E2 từ bundle tĩnh: có luồng WebSocket realtime, worker sync-v2 riêng và media bất đồng bộ. Chưa có E1 chứng minh ưu tiên scheduler, echo về socket gửi, hay thứ tự server. Do đó kế hoạch áp dụng ưu tiên nội bộ cho realtime/outgoing/call so với history, nhưng không tuyên bố đó là hợp đồng hoặc hành vi scheduler đã xác minh của Zalo. Không sửa params, signing, frame IDs hay cursor semantics ngoài nguyên tắc đã được ADR/re-notes chứng minh.

## Trạng thái hiện tại và nguyên tắc làm việc

Khi lập kế hoạch, working tree đã có các diff chưa hoàn tất trong `src/gui/worker.rs`, `src/gui/workspace.rs`, `src/outgoing.rs`, `src/service_resolver.rs`, `re-notes/44-map15-network-send.md` và `docs/ZALO_REFERENCE_ANALYSIS.md`. Chúng khi ấy là **patch đang dở**, không phải kết quả đã sửa hoặc đã kiểm chứng. Các tài liệu Claude/Codex trước đây là phân tích tĩnh; không thay cho kiểm tra lại diff, test hay đo trên thiết bị. Kết quả triển khai hiện tại được ghi dưới đây; các diff ngoài phạm vi P0/P1 được giữ nguyên.

Trước khi sửa, phân loại từng hunk theo yêu cầu hiện tại và giữ nguyên thay đổi ngoài phạm vi. Không build lại chỉ để mở ứng dụng: dùng binary sẵn ở `C:\Apps\ZaloLite` làm baseline và kiểm tra. Binary đó không đại diện cho patch mới. Mọi thay đổi protocol phải theo `AGENTS.md`, `docs/ZALO_REFERENCE_ANALYSIS.md` và bằng chứng `re-notes/`; ghi rõ Parity/Fork deviation/Unknown.

## Kết quả triển khai P0/P1 (2026-11-21)

**P0 — Một phần; có audit trực tiếp trên debug build nhưng chưa đủ nghiệm thu.** Đã build và mở bản ARM64 debug từ source hiện tại, rà chat, search, drawer, danh bạ, cài đặt, composer, picker và gửi vài tin thử tới Cloud của chính tài khoản. Tái hiện được dải đen giữa sidebar/chat, cảnh báo SQLite/cursor, picker không hiện để dùng và các vấn đề đồng nhất UI. Hai snapshot gửi đơn cho trạng thái “Đã gửi” sau 848ms/1,014ms tính cả overhead input/chụp màn hình; burst bàn phím cho bubble text trộn/ghép. Những lượt burst/modifier qua automation có thể khác input người thật, nên cần tái hiện bằng bàn phím vật lý trước khi quy nguyên nhân. Có một mẫu CPU/RAM idle trên debug build; chưa có frame p95, ingress→render, request→ACK/echo, queue depth hoặc DB timing. Chưa hoàn tất media/call, human IME, resize hẹp hay đối chứng Zalo PC cùng viewport. Vì vậy chưa kết luận latency server hoặc nghiệm thu UX hoàn tất. Xem [báo cáo P0](2026-11-21-p0-baseline-verification.md) và [audit UI trực tiếp](2026-11-21-native-test-build-ui-audit.md).

**P1 — Hoàn tất ở mức source và automated checks; hai audit độc lập Accept.** Outgoing intent được gắn owner/runtime tại lúc nhận lệnh; ticket phát terminal result khi intent bị từ chối hoặc task bị hủy (`NotSent` trước transport, `DeliveryUnknown` sau khi đã bắt đầu). Pending được phát trước I/O; việc lưu Pending và outcome cuối được tuần tự để ACK không bị ghi lùi. QR/password/logout/profile switch/expiry xác minh generation trước khi thay đổi session. Account DB, metadata/cache, history memo, cursor và session-file writes dùng permit; logout drain writer cũ trước khi kết thúc generation. `session.json` removal được scope theo runtime đã ghi file. TUI logout cũng drain trước cleanup.

Hai subnet audit độc lập đã đồng thuận Accept trên các invariant P1. Trên Windows ARM64 hiện tại: `cargo check --tests --locked -j 1` qua; `cargo test --locked --lib -j 1` cho **999 passed, 0 failed, 10 ignored**; `cargo clippy --all-targets --locked -j 1 -- -D warnings` qua. `cargo fmt --all -- --check` và `git diff --check` qua. Audit và unit tests không thay cho Zalo end-to-end/device acceptance.

**Giới hạn còn lại:** cursor có thể không flush khi gate đã seal; file cursor cũ được giữ nên restart có thể replay trang đã lưu. Message persistence idempotent làm điều này an toàn về dữ liệu, nhưng có thể tăng công việc sync. Không có thay đổi contract/params/frame IDs; quyết định scheduler của Zalo vẫn là Unknown cho tới khi có E1.

## Các giai đoạn

### Cập nhật triển khai P2–P4

Xem [báo cáo triển khai và kiểm chứng](2026-11-21-p2-p4-implementation-verification.md). P2 đã bỏ full snapshot trên từng Pending/ACK, giữ tên/metadata bằng targeted command; P3 đã tách bounded control/typing priority, reaction drain và tra tên, giữ cursor theo durable page. Có P5a composer, picker và layout blocker đi kèm theo lộ trình đã chốt. Hai audit độc lập A/B Accept mã; library tests 1.021 passed/0 failed/11 ignored, Clippy qua. Release ARM64 đang build, chưa đóng P4 trước kết quả binary/đo native thật.

### P0 — Chốt baseline và số đo an toàn

1. Quét lại diff và mã thực tế; xác định đường vào/ra, nơi event được tạo, nơi SQLite/cache ghi và mọi nhánh login/logout/switch. Không dựa vào log hội thoại cũ.
2. Chạy binary hiện có để ghi baseline trải nghiệm (nếu app/tài khoản đang mở): gửi liên tiếp, nhận tin trong lúc sync, group policy loading, media. Không gửi nội dung thử nghiệm ngoài phạm vi người dùng đã cho trước đó; không lưu nội dung, ID, cookie, URL hay thông tin phiên vào số đo.
3. Thêm/ dùng bộ đếm timing có allowlist: Enter→Pending, worker dequeue→request start, request→ACK/echo, ingress frame→UI event, transaction duration/busy wait, `PersistThreads` duration/clone cost, event queue depth và egui frame p95. Đo bằng fixture SQLite và trên ARM64; ghi rõ metric nào chưa đo được.
4. Phân biệt `Pending` ngay lập tức, ACK, echo, `DeliveryUnknown`, `NotSent` và lỗi server. Timeout mơ hồ không được tự biến thành “chưa gửi” hoặc retry mutation.

**Cổng qua P0:** baseline có kịch bản lặp lại được, số đo không chứa dữ liệu riêng tư, và từng diff dở có quyết định giữ/sửa/bỏ được ghi rõ. Nếu chưa thể dùng tài khoản thật thì tiếp tục fixture, ghi giới hạn, không tuyên bố native acceptance.

### P1 — Đúng identity và kết quả cuối; đóng ranh giới phiên

1. Sửa outgoing sender để mỗi intent mang owner/runtime tại thời điểm người dùng gửi; không chụp identity cũ trong sender sống qua QR/password login. Kiểm tra riêng QR login, password login, logout, A→B và A logout→A login lại.
2. Mọi lệnh bị từ chối do runtime nguồn đóng phải có kết quả terminal `NotSent` gắn identity nguồn. Không để receiver `continue` im lặng; UI phiên khác được phép bỏ event cũ sau khi nguồn đã có terminal outcome.
3. Bảo đảm admission `Pending` và ACK đến đảo thứ tự vẫn reconcile đúng theo Local message ID, không tạo dòng ma/duplicate và không mất ACK. `mark_queued` trong RAM không được mô tả như durable outbox.
4. Thực hiện barrier nghiêm ngặt cho write của account quanh logout/switch: writer giữ quyền đến commit/abort; boundary chờ writer kết thúc trước khi tăng generation/đổi session. Timeout chỉ đưa UI vào trạng thái chờ và có thể hủy thao tác chuyển phiên; không bỏ qua guard. Bao phủ mọi đường ghi liên quan, không chỉ outgoing.
5. Thêm regression race cốt lõi: A bắt đầu write, logout A chờ, A đăng nhập runtime mới, write cũ hoàn tất trễ; chứng minh không có commit generation cũ sau khi generation mới hoạt động trong cùng DB A. Thêm A→B và QR/password tương ứng.

**Cổng qua P1:** không có send bằng session sai, không có lệnh âm thầm mất, mọi outcome đều terminal, test race pass. Nếu barrier làm phiên mới không thể mở vì writer treo, phải tìm cancellation/drain có xác nhận abort hoặc coordinator; tuyệt đối không chuyển sang timeout phá invariant.

### P2 — Xử lý nút thắt gửi và lưu sidebar dựa trên số đo

1. Xác nhận bằng timing liệu `apply_outgoing_state` có clone toàn bộ `threads` và gửi `PersistThreads` mỗi event; đo thời gian clone, queue delay và SQLite/cache write.
2. Nếu được xác nhận, bỏ full-snapshot persistence khỏi từng outgoing event. Giữ cập nhật unread/last message/sidebar sau restart. Dùng snapshot sequence/version hoặc cơ chế coalesce có thứ tự để write cũ không ghi đè metadata mới. Vì JSON cache là một file, đo read-modify-write trước khi gọi đó là incremental persistence.
3. Tách lưu message row theo từng thay đổi cần thiết khỏi ghi metadata sidebar; không thêm schema migration nếu không cần. Nếu cần migration, theo database standard và chỉ thêm sau khi có thiết kế/fixture rõ.
4. Giữ text không bị upload/download/thumbnail chặn. Đo file/ảnh/video riêng; chỉ thêm giới hạn concurrency/priority nếu metric cho thấy starvation hoặc quá tải.

**Cổng qua P2:** Enter→Pending tức thời; không còn QueueWait giả làm lỗi server; burst text không mất intent; sidebar và message state khôi phục đúng sau restart; stale snapshot không ghi đè state mới.

### P3 — Tách WebSocket ingress khỏi công việc phụ mà vẫn giữ cursor an toàn

1. WebSocket reader chỉ decode/classify và đẩy event vào queue bounded; không chờ HTTP tra tên, SQLite chậm, reaction drain hay media. Nếu queue durable đầy, áp backpressure có chủ đích và giữ cursor cũ/replay; không drop frame âm thầm và không spawn task vô hạn.
2. Tách `resolve_thread_names`/profile fetch sang tác vụ nền có cache; tin vẫn render bằng thông tin hiện có. Không để tên chưa tra được chặn call hoặc message.
3. Reaction: enqueue bền vững trong cùng điều kiện an toàn của page trước cursor; drain/render riêng và ở ưu tiên thấp hơn message/call. Lỗi enqueue giữ cursor; lỗi drain không làm reader kẹt vô hạn.
4. Xác định rõ các terminal result persistence: page lưu + cursor commit, page lưu nhưng cursor hoãn, hoặc persistence fail + cursor giữ nguyên. Duy trì idempotency khi replay theo `(thread_id, msg_id)`.
5. Call control và outgoing không chờ history sync, reaction rendering, profile lookup hay media preview. History vẫn có concurrency cap/fairness để không chết đói, nhưng không chiếm lock/worker cần cho realtime.

**Cổng qua P3:** failure injection cho DB chậm/lỗi, reaction enqueue/drain lỗi, reconnect/replay và burst messages chứng minh không mất page, cursor không tiến sớm, reader không bị việc phụ giữ lại, queue có giới hạn.

### P4 — Đo E1 và nghiệm thu trên Windows ARM64

1. Dùng binary đã build tương ứng với source được kiểm tra (chỉ build khi cần xác minh patch mới và sau khi xác nhận toolchain); không nhầm binary cài sẵn với source hiện tại.
2. So sánh p50/p95 Enter→Pending, Enter→ACK/echo; frame→bubble; frame p95; queue depth và DB busy dưới tải thường, burst liên tục, history sync, reconnect/replay.
3. Trên thiết bị/tài khoản thật: gửi liên tục; nhận tin; group “đang xác minh quyền gửi”; chạy sync; gửi/nhận file, ảnh, video; xác nhận gọi đến/đi không bị history/media chặn. Không khẳng định thứ tự server nếu không quan sát được echo E1.
4. Dùng Zalo PC mở cạnh fork để đối chiếu cảm nhận và hành vi cần thiết. Ghi build, kịch bản, số đo và giới hạn; tuyệt đối không ghi nội dung tin hoặc bí mật.

**Cổng qua P4:** báo cáo rõ fixture/test, compile, benchmark và native-device acceptance thành các mức riêng. Compile bị chặn bởi `ring`/`clang.exe` phải ghi là blocked, không xem test cũ là xác nhận hiện tại.

### P5 — Hoàn tất backlog UI/UX độc lập

Sau khi latency path ổn định, xử lý từng mục và xác minh trên Windows ARM64:

1. Composer: Shift+Enter xuống dòng; Enter gửi; `Ctrl+Shift+S` mở text editor (và xác nhận `Ctrl+Shift+X` nếu đó là shortcut hiện có); kiểm tra IME tiếng Việt, focus, multiline và accessibility.
2. Bố cục: khoảng trống dưới composer; search không kéo giãn sidebar; dải đen khi search/sync; clipping, width constraints và light/dark theme.
3. Groups: avatar ghép khi nhóm chưa có ảnh, nguồn thành viên/hoạt động phải được xác minh; đồng bộ alias UID→tên xuyên group và phân biệt contact alias với profile name.
4. Timeline: reaction không lấn át message; hiển thị tên người reaction khi payload/cache có bằng chứng sender; nếu thiếu danh tính thì biểu diễn trung tính, không bịa tên.
5. Typography: emoji/heart không thành đen, khoảng cách glyph đúng; tên “Bống” theo đúng nguồn alias/profile. Đối chiếu font fallback và metrics trên ARM64, không sửa bằng khoảng trắng thủ công.

Avatar ghép, alias group và reaction identity cần parity evidence từ `re-notes/`/bundle hoặc live capture trước khi thay đổi hợp đồng dữ liệu. Lỗi render thuần túy có thể sửa độc lập sau khi nắm đúng dữ liệu đầu vào.

## Kiểm thử và báo cáo

- Theo chuẩn tương ứng trong `docs/standards/`: concurrency/locking, API/error handling, database/migrations và testing/fixtures.
- Từng regression gắn một invariant quan sát được; test không được chỉ mô phỏng implementation. Ưu tiên test identity/session, ACK đến trước/sau Pending, queue bounded, cursor replay và metadata snapshot ordering.
- Chạy `cargo fmt --all -- --check`, `git diff --check`, test liên quan và `powershell scripts/audit-prepush.ps1` (hoặc lệnh được `AGENTS.md` cho phép) trước commit/push. Chưa có ủy quyền commit/push/release trong yêu cầu này.
- Mỗi báo cáo ghi chính xác: **chưa làm / một phần / đã có mã / qua kiểm thử / xác nhận trên thiết bị**. Không dùng lịch sử hội thoại hoặc tài liệu cũ để thay thế bằng chứng mới.

## Thứ tự ưu tiên tóm tắt

`P0 baseline + audit diff` → `P1 identity/result/barrier` → `P2 đo và sửa persistence/outgoing` → `P3 ingress/cursor/reaction` → `P4 ARM64 acceptance` → `P5 UI/UX`.

Không cần thêm vòng phản biện Claude cho thiết kế này. Các giả thuyết về scheduler của Zalo PC vẫn được đánh dấu E2/E3; quyết định nội bộ ở đây dựa trên lỗi quan sát, invariant của repo và số đo cần thu thập.
