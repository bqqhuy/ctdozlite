# Báo cáo hợp nhất 21 phát hiện và kế hoạch khắc phục

**Dự án:** ctdozlite / ZaloLite, v0.016-arm.  
**Ngày:** 16/11/2026, Asia/Saigon.  
**Snapshot:** nhánh `main`, commit `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`.  
**Phạm vi lượt này:** hợp nhất báo cáo và lập kế hoạch; chưa sửa code, tests, schema, dependencies hay cấu hình; chưa xóa code, commit, push hoặc release.

## 1. Kết luận và cách sử dụng

Danh mục cuối gồm **21 phát hiện: 3 P1, 14 P2, 4 P3**. Cả 21 mục đều **chưa khắc phục/chưa nghiệm thu sau sửa**. Không có P0 đủ bằng chứng trong tập đã công bố; điều này không chứng nhận toàn bộ sản phẩm không có P0 khác.

Tài liệu này trình bày đầy đủ vấn đề, bằng chứng, giới hạn, hướng khắc phục và cách kiểm chứng từng mục. Có thể dùng làm tài liệu bàn giao triển khai mà không phải đọc nối hai báo cáo trước. Các báo cáo độc lập và log vẫn được giữ để truy vết, không phải các danh mục lỗi cộng thêm.

Ba reviewer `security`, `data_logic`, `gui_quality` đã audit độc lập và phản biện chéo: thống nhất F01–F17 trong audit đầu, sau đó thống nhất F18–F21 qua phản biện Antigravity. Báo cáo hợp nhất giữ ID, severity và giới hạn đã chốt; không tiếp nhận nguyên trạng nhãn CRITICAL/P0 hay action plan của tài liệu ngoài.

**Rà soát bản hợp nhất:** cả ba reviewer đã đọc toàn bộ tài liệu này và chấp thuận đủ 21 mục, phân đợt A–D, kế hoạch sửa và tiêu chí kiểm chứng; không còn yêu cầu chỉnh sửa bắt buộc. Kiểm tra cấu trúc xác nhận F01–F21 xuất hiện đúng một lần ở phần chi tiết và mỗi mục đều có hướng khắc phục, kiểm chứng, tiêu chí đóng. HEAD/source/index diff giữ nguyên; lượt này chỉ thêm tài liệu, không chạy lại Cargo.

### Mức ưu tiên và loại bằng chứng

| Nhãn | Ý nghĩa |
|---|---|
| P1 | Cần sửa trước khi tin cậy các luồng riêng tư bị ảnh hưởng: owner audio, PIN hội thoại ẩn, xóa tin. |
| P2 | Lỗi chức năng, dữ liệu, khả năng phục hồi hoặc rủi ro có điều kiện; một số mục chỉ thuộc E2EE thử nghiệm. |
| P3 | Vi phạm nguyên tắc, chất lượng công cụ hoặc khả năng bảo trì; không tự đồng nghĩa có exploit. |
| Nguồn | Chứng minh luồng theo code hiện tại; chưa nhất thiết tái hiện qua ứng dụng đang chạy. |
| Mô hình | Thực thi mô hình SQLite/Python offline; không phải thực thi Rust/GUI/native tương ứng. |
| E1/E2/E3 | Riêng parity Zalo: capture thật / đọc bundle tĩnh / suy luận. Đọc source fork không tự thành E1 hoặc E2 của Zalo. |

Mỗi mục dưới đây có **kiểm chứng đề xuất** cho lượt sửa sau. Không được hiểu đó là test đã chạy, test hiện đã tồn tại hay tiêu chí đã đạt. Unit/fixture pass không thay nghiệm thu native ARM64 hoặc interoperability Zalo.

## 2. Danh mục và kế hoạch theo ưu tiên

| ID | Mức | Nội dung | Đợt xử lý |
|---|---|---|---|
| F01 | P1 | Audio/micro không ràng buộc tài khoản và hội thoại | A |
| F02 | P1 | TUI bỏ qua PIN hiển thị hội thoại ẩn | A |
| F03 | P1 | Tin xóa trong TUI có thể hồi sinh từ DB | A |
| F04 | P2 | Event cũ có thể hiện lại dữ liệu sau logout | A |
| F05 | P2 | TUI mất ACK và định danh lỗi gửi | B |
| F06 | P2 | SQLite/cache đồng bộ chặn UI TUI | D; phần cần cho A làm sớm |
| F07 | P2 | Migration 13 nuốt lỗi và không atomic | A, trước thay đổi persistence |
| F08 | P2 | Gỡ reaction nhưng phát lệnh thêm | B |
| F09 | P2 | Voice clone mỗi frame, xử lý audio trên UI | D, sau ownership F01 |
| F10 | P2 | Assurance E2EE vượt trạng thái và bằng chứng | B |
| F11 | P2 | WebSocket write thiếu deadline/cancellation | B |
| F12 | P2 | QR verbose có thể in dữ liệu nhạy cảm | B, làm sớm |
| F13 | P2 | Binding signing identity và DH identity chưa được kiểm | C, cần evidence trước thiết kế |
| F14 | P3 | Mở diagnostics làm I/O trực tiếp trên UI | D |
| F15 | P3 | Salt đăng nhập dùng PRNG thời gian | D; có thể làm độc lập sớm |
| F16 | P3 | Fallback sticker host nằm ngoài endpoints | D; có thể làm độc lập sớm |
| F17 | P3 | Prepush quảng bá coverage chưa thực sự kiểm | D; hoàn thành trước release tiếp theo |
| F18 | P2 | Parser thiếu nhận diện sender=self | B |
| F19 | P2 | Fingerprint local không khớp scheme Signal đã tuyên bố | C, cần vector/parity evidence |
| F20 | P2 | Menu tài khoản trùng rail và overlay | D |
| F21 | P2 | Windows Hello thiếu desktop HWND interop và recovery có giới hạn | B |

### Đợt A — Ranh giới riêng tư và tính bền vững dữ liệu

**Phạm vi:** F01, F02, F03, F04, F07. Kiểm migration F07 trước khi thay persistence; đồng thời sửa audio ownership và hidden gate. F03 phải dùng đường DB nền, có thể triển khai phần tối thiểu của F06 sớm để không tạo thêm I/O trên UI.

**Điều kiện kết thúc:** không audio/event vượt owner; TUI không lộ hidden content khi chưa unlock; delete được lưu và chống replay hồi sinh; migration fail không công bố version mới. Các regression offline tương ứng phải pass. Native audio/PIN trải nghiệm thật được ghi riêng, không đánh hoàn tất bằng mock.

### Đợt B — Outcome, logging và khả năng phục hồi

**Phạm vi:** F12, F10, F05, F08, F18, F11, F21. Ưu tiên redaction và assurance ngay; tiếp theo sửa send outcome/reaction/direction, rồi transport và desktop authentication. Các thay đổi owner/generation của A là nền cho kiểm stale completion ở B.

**Điều kiện kết thúc:** không raw auth log; không assurance vượt capability; send/reaction phản ánh đúng kết quả; self message qua adapter đã kiểm có direction đúng; write và auth có kết thúc hữu hạn; Windows Hello được kiểm trên desktop ARM64.

### Đợt C — E2EE validation và fingerprint có bằng chứng

**Phạm vi:** F13, F19. Thu thập/phân tích evidence có thể bắt đầu sớm song song A/B; việc này không cho phép đoán hợp đồng rồi viết crypto.

**Điều kiện kết thúc:** scheme/binding có provenance và vector độc lập; negative cases bị từ chối trước mutation; fingerprint đúng scheme đã lựa chọn. Nếu thiếu bằng chứng Zalo thì giữ trạng thái chưa nghiệm thu parity, giữ gate thử nghiệm và hiển thị đúng giới hạn. Không “đóng lỗi interoperability” chỉ bằng roundtrip giữa hai instance fork.

### Đợt D — Responsiveness, cấu trúc UI và công cụ

**Phạm vi:** F06, F09, F20, F14–F17; sau đó mới tối ưu thêm và dọn candidate code ở mục 5. F15/F16/F17 có thể là thay đổi nhỏ độc lập, không cần chờ mọi mục C hoàn tất. Đây là nhóm công việc theo rủi ro, không phải lịch tuần tự cứng hoặc ước tính thời gian đã cam kết.

**Điều kiện kết thúc:** I/O nặng rời UI; audio resource có giới hạn; một menu account duy nhất; random/host/gates đúng chuẩn; các tối ưu có đo trước/sau, không đổi semantics.

## 3. Chi tiết từng phát hiện và kiểm chứng

### F01 — P1: Audio/micro vượt ranh giới owner hoặc hội thoại

**Bằng chứng:** `src/gui.rs:1020`, `:2921`; `src/gui/workspace.rs:6992`, `:7025`. Recorder, preview và samples là state toàn app; logout/profile switch không hủy chúng. Send lấy buffer cũ nhưng dùng thread và worker hiện tại.

**Trigger/tác động:** A thu hoặc giữ audio chưa gửi, chuyển B rồi bấm gửi có thể gửi audio A qua B. Đổi chat cũng có thể đổi đích gửi. Micro có thể tiếp tục thu sau logout. Đây không phải cáo buộc tự động gửi khi chưa có thao tác người dùng. Bằng chứng hiện có là nguồn, chưa thu âm/chạy native để tái hiện.

**Khắc phục đề xuất:** gắn account runtime/generation và thread identity cho lifecycle audio; cancel recorder/preview, xóa samples khi logout/đổi account. Khi chuyển chat phải hủy hoặc có cơ chế giữ draft gắn đích rõ ràng; send luôn xác minh owner/đích. Kết quả async cũ phải bị bỏ.

**Kiểm chứng:** mock audio backend ghi nhận start/stop/drop; tạo PCM giả cho A/X, lần lượt logout, switch B, chuyển Y rồi inject completion cũ. Assert không có SendVoiceMessage mang PCM đó tới B/Y và recorder được dừng khi owner đổi. Kiểm thêm send hợp lệ A/X vẫn thành công, cancel/preview không rò resource. Native ARM64 kiểm đèn/quyền micro, preview, restart và switch thật bằng tài khoản thử nghiệm.

**Tiêu chí đóng:** lifecycle tests pass; không có audio vượt owner/đích ngoài quyết định draft đã chốt; micro ngừng theo chính sách đã công bố. Native acceptance được ghi rõ.

### F02 — P1: TUI không áp dụng hidden-chat PIN display gate

**Bằng chứng:** `src/tui.rs:201`, `:258`; `docs/adr/0034-hidden-chat-pin-compatibility.md`. TUI hydrate/filter/open không dùng gate hidden membership mà GUI thực thi.

**Trigger/tác động:** có lịch sử hội thoại ẩn trên máy, mở TUI khi chưa unlock có thể thấy nội dung. Đây là bypass riêng tư hiển thị trong app; không phải phá mã hóa SQLite hoặc bảo vệ trước raw filesystem access.

**Khắc phục đề xuất:** dùng cùng owner-bound membership/Vault và policy cho list, search, open/history cùng mọi đường trình bày nội dung. Unknown membership phải được xử lý fail closed đúng ADR; logout/restart làm mất trạng thái unlock. Không tạo PIN verifier riêng lệch GUI.

**Kiểm chứng:** fixture gồm chat thường, hidden direct/group và membership unknown. TUI list/search/open không lộ tên/preview/content hidden khi locked; wrong PIN không unlock; đúng PIN mới hiện theo policy. Restart offline, logout và switch account không kế thừa unlock. Chat thường được xác minh là nonhidden vẫn dùng được theo ADR.

**Tiêu chí đóng:** mọi đường TUI chạm private content đều được gate; regression locked/offline/unknown/account-switch pass. Kiểm GUI/TUI parity bằng fixture trước rồi nghiệm thu native UX.

### F03 — P1: Xóa tin TUI chỉ xóa vector, hàng DB còn lại

**Bằng chứng:** `src/tui.rs:1478`, `:201`; `src/store.rs:1594`. DeleteDone bỏ tin khỏi memory rồi persist snapshot kiểu upsert, không delete/tombstone hàng vắng. Mô hình SQLite đã tái hiện còn 1 row sau khi view có 0 tin; chưa chạy Rust delete integration tương ứng.

**Khắc phục đề xuất:** reuse delete-for-me/tombstone theo canonical identity trên DB worker; transaction gồm marker và trạng thái cần thiết. Chống stale poll/history replay đưa lại tin. Phân biệt server ACK với lưu local thất bại, không tuyên bố local deletion đã bền vững khi chưa lưu. Giữ nguyên semantics recall khác delete-for-me.

**Kiểm chứng:** store in-memory với tin canonical và alias; áp dụng delete qua boundary thật, hydrate lại, replay same message/alias/history; tin không trở lại và thread khác không mất dữ liệu. TUI event test phải chứng minh gọi đúng persistence operation, không chỉ retain vector. Inject DB write failure sau server ACK để kiểm status/recovery. Reopen lifecycle dùng fixture temp cô lập khi thực sự cần kiểm persistence qua process.

**Tiêu chí đóng:** delete→hydrate/restart→replay không hồi sinh; idempotent; lỗi persistence có kết quả trung thực. Mô hình Python cũ không thay regression Rust này.

### F04 — P2: Event TUI cũ áp dụng sau logout

**Bằng chứng:** `src/tui.rs:988`, `:1393`, `:1406`. Logout clear state nhưng queued PollDone/ThreadsRefreshed không có owner/generation guard.

**Trigger/tác động:** response xếp hàng trước logout, xử lý sau logout có thể hiện lại tin cũ. Cache guard đã ngăn ghi khi owner rỗng; không kết luận ghi nhầm anonymous cache.

**Khắc phục đề xuất:** runtime generation/account identity trong background work và completion; reject event stale trước mọi mutation/render/persist, signal cancel và drain có giới hạn. Review cả history, attachment và action completion, không chỉ hai variant đã lộ lỗi.

**Kiểm chứng:** enqueue poll/refresh cho A, logout rồi consume; state phải vẫn rỗng. Đăng nhập lại A ở generation mới hoặc B rồi inject event A cũ cũng bị bỏ; completion hiện tại vẫn hoạt động. Assert không tăng unread, đổi selection/title hoặc tạo DB operation từ stale event.

**Tiêu chí đóng:** owner/generation isolation có test tại event boundary, không chỉ test helper equality; shutdown không chờ task vô hạn.

### F05 — P2: TUI mất ACK và định danh lỗi gửi

**Bằng chứng:** `src/tui.rs:804`, `:834`, `:1508`. Optimistic message được lưu nhưng ACK bị map thành unit; SendFailed chỉ mang chuỗi, handler chỉ đổi status.

**Tác động:** server từ chối nhưng row local vẫn tồn tại không có trạng thái lỗi đúng; khó reconcile/retry chính xác. Timeout có thể là kết quả chưa biết, không chứng minh chưa giao.

**Khắc phục đề xuất:** mang owner/generation/client ID ổn định và typed outcome/ACK qua event; reconcile đúng row, persist delivery state và retry payload cần thiết. Phân biệt acknowledged, rejected và unknown; retry không sinh ID mới hoặc tự resend mutation chưa xác định.

**Kiểm chứng:** hai tin gửi song song, đảo thứ tự ACK/error; chỉ row đúng được cập nhật. Fixture cho success, explicit rejection, timeout-after-server-accept, echo-before-ACK và restart. Assert không duplicate, không đánh tin A failed vì lỗi B, unknown không thành sent/rejected vô căn cứ, retry giữ identity.

**Tiêu chí đóng:** end-to-end TUI→background→ACK/outcome→state→DB có payload thật và các race trên đều pass.

### F06 — P2: SQLite và cache I/O đồng bộ trên UI TUI

**Bằng chứng:** `src/tui.rs:477`, `:825`, `:1418`, `:1483`; busy timeout `src/store.rs:87` là 5 giây. Send/history/delete gọi disk/DB trên đường xử lý render/input.

**Khắc phục đề xuất:** một writer/background queue sở hữu DB và cache writes; UI gửi intent, nhận terminal event. Coalesce/bound nguồn phát để không chuyển UI freeze thành queue tăng vô hạn; giữ ordering, account token và shutdown flush có giới hạn. Không dùng thêm threads không quản lý cho mỗi row.

**Kiểm chứng:** synthetic writer lock hoặc I/O fault giữ DB operation; UI vẫn xử lý được input/event khác trước khi lock được nhả. Kiểm success/error/cancel, ordering send-delete-replay và shutdown khi còn writes; không bỏ message vì ghi history thất bại. Đo p95/p99 input/frame latency và queue depth trên dataset cố định trước/sau.

**Tiêu chí đóng:** không còn SQLite/cache write đồng bộ ở các UI path nêu trên; test responsiveness/ordering/error pass và số đo có môi trường/dataset, không chỉ “có spawn”.

### F07 — P2: Migration 13 nuốt lỗi và công bố version không atomic

**Bằng chứng:** `src/store.rs:228`–247. Introspection/ALTER/PRAGMA lỗi bị bỏ qua; ALTER và bump version không cùng transaction. Mô hình SQLite fault-injection đã tái hiện version 13 thiếu cột, chưa tái hiện bằng Rust migration thật.

**Khắc phục đề xuất:** kiểm schema strict, transaction cho bước migration, propagate lỗi; chỉ bump sau khi postcondition cột tồn tại đúng. Khi mở schema 13 bị thiếu cột phải nhận diện rõ và có phương án recovery được thiết kế, không silently tiếp tục. Không xóa DB hoặc reset history để né lỗi.

**Kiểm chứng:** schema12 sạch→13 có dữ liệu; schema12 đã có cột; schema13 hợp lệ; schema13 thiếu cột; version mới hơn app. Inject failure ở introspection/ALTER/bump theo harness phù hợp, kiểm rollback/version/data invariants. Chạy lại sau failure phải an toàn. Store unit dùng in-memory; fixture temp chỉ cho lifecycle cần reopen, không DB người dùng.

**Tiêu chí đóng:** migration lỗi không trả thành công với postcondition sai, không mất lịch sử, không bump dở; recovery cho DB đã sai version được kiểm riêng. Không yêu cầu thêm production dependency chỉ để thực hiện fault injection.

### F08 — P2: Gỡ reaction nhưng vẫn phát nonempty reaction

**Bằng chứng:** `src/gui/workspace.rs:3050`; `src/chat.rs:4263`. UI clear local nếu reaction đã có nhưng WorkerCmd vẫn gửi emoji; backend chỉ empty hoặc `/-clear` mới là removal.

**Khắc phục đề xuất:** xác định add/remove intent một lần, dùng đúng contract đã có; update state từ completion có identity/revision. Nếu optimistic thì rollback có scope; không xóa reaction của người khác hoặc ghi đè response mới hơn.

**Kiểm chứng:** click thêm emoji rồi click lại; command thứ hai phải là removal đúng định dạng. Capture request fixture xác nhận rType/add/remove; inject reject/timeout, concurrent peer reaction và stale completion. Canonical reaction state và render phải đồng nhất.

**Tiêu chí đóng:** command-dispatch + request + event/state regression pass. Không suy live server xử lý được chỉ từ mock; nếu chỉnh protocol phải đối chiếu evidence hiện có và ghi parity.

### F09 — P2: Audio clone mỗi repaint và xử lý nặng trên UI

**Bằng chứng:** `src/gui/workspace.rs:3754`, `:6992`, `:7038`; `src/voice.rs`. Clone toàn samples ở repaint, setup thiết bị/resample/WAV ở UI; chưa có giới hạn thời lượng rõ. Chưa có FPS/RAM benchmark native cho lỗi này.

**Khắc phục đề xuất:** sau F01, dùng ownership/immutable snapshot phù hợp, chỉ copy khi hành động cần; device setup/encode ở worker/blocking pool; quy định thời lượng/byte budget và kết thúc recording có thông báo. Không tự chọn quota vô căn cứ hoặc giấu việc cắt audio.

**Kiểm chứng:** synthetic PCM 1/10/30 phút; đo bytes allocated/repaint, peak memory và input latency; frame không clone theo độ dài audio khi idle. Test resample/WAV output bằng fixture, rate/channel khác nhau, cancel trong encode, vượt quota, logout trong task. Native ARM64 kiểm micro/preview/send, không dùng mock để chứng nhận device.

**Tiêu chí đóng:** resource bound được công bố và thực thi; O(samples) copy không lặp theo mỗi frame; audio output không đổi và benchmark trước/sau được lưu. Không hứa 60 FPS khi chưa đo.

### F10 — P2: Drawer đưa assurance E2EE không dựa trạng thái thật

**Bằng chứng:** `src/gui/workspace.rs:6669`; `src/e2ee_trial.rs:16`; `src/gui/worker.rs:1357`. Text bảo vệ/“toàn diện” chỉ dựa trusted_peers hoặc hiện vô điều kiện, không xét build/runtime/session/group/media.

**Khắc phục đề xuất:** model hiển thị từ capability, required-policy, active/pending/failed crypto state và phạm vi nội dung. Verified identity không đồng nghĩa mọi message đã encrypted hoặc mọi media/group được hỗ trợ. Thiếu parity phải hiển thị thử nghiệm/chưa xác minh; dùng tiếng Việt đúng chuẩn repo.

**Kiểm chứng:** ma trận build không feature/có feature, runtime off/on, peer unknown/required, session missing/pending/active/failed, identity trusted/changed, direct/group/media. Assert không có success assurance ở unsupported/failed/off cases; trust badge không tự nâng bảo vệ toàn hội thoại.

**Tiêu chí đóng:** state→render tests pass và native kiểm nội dung hiển thị; không đợi F13/F19 xong mới sửa assurance sai, nhưng không claim official parity trước nghiệm thu.

### F11 — P2: WebSocket write thiếu deadline và cancellation

**Bằng chứng:** `src/socket.rs:637`, `:857`, `:898`, `:974`, `:1017`. Một số send nằm trong selected branch; khi await bị Pending, select không poll tx.closed/liveness tiếp. Outer task không có bound thay thế.

**Khắc phục đề xuất:** deadline và cancellation thống nhất cho mọi write; quản lý task lifecycle/shutdown/reconnect. Write timeout có thể là delivery unknown, không tự resend mutation. Không đổi command IDs, heartbeat contract hoặc retry policy khi chưa có evidence.

**Kiểm chứng:** fake sink giữ Pending riêng heartbeat/Pong/initial ping/GET_MSG; trigger deadline, receiver drop và shutdown. Assert task kết thúc/reconnect theo budget đã định, không leak task hoặc spin. Healthy sink, partial write/error và unknown frame vẫn đúng. Dùng clock/harness có kiểm soát, không dựa việc chờ mạng thật bị treo.

**Tiêu chí đóng:** mọi write path có bound và cancellation tests pass; native/network soak kiểm reconnect riêng, không coi loopback là bằng chứng mạng thật.

### F12 — P2: QR verbose có thể làm lộ auth response

**Bằng chứng:** `src/qr.rs:404`–410. Raw body 200 ký tự được in trước redaction, cookie còn prefix. Truncate không phải scrub. Chưa xác nhận một sự cố lộ credential thực tế.

**Khắc phục đề xuất:** log metadata/status/error code có cấu trúc; không in raw auth body hoặc secret prefix. Nếu dùng sanitizer thì scrub trước truncate và xử lý nested/string-encoded JSON, malformed response. Dev/verbose không miễn quy tắc secrets.

**Kiểm chứng:** synthetic response/cookie chứa sentinel giả cho token, zpw_sek/zpw_enk, IMEI, phone/name; đặt đầu/cuối/nested/encoded, bật mọi verbose mode. Capture đúng stdout/stderr/log boundary và assert không có sentinel hay prefix nhạy cảm, nhưng status/error vẫn hữu ích. Không đưa credential thật vào test.

**Tiêu chí đóng:** toàn bộ QR response logging path không phát dữ liệu nhạy cảm; test output thực chứng minh, không chỉ test sanitizer không được caller dùng.

### F13 — P2 experimental: Chưa chứng thực binding signing key với DH identity

**Bằng chứng:** `src/crypto/prekey.rs:79`; `src/crypto/session.rs:316`, `:558`. SPK được verify bằng signing_identity_key tự cung cấp; X3DH/trust dùng identity_key khác mà không kiểm proof liên kết.

**Giới hạn:** lỗi local bundle validation trong thử nghiệm; chưa chứng minh MITM/decryption trái phép hay server exploit. Hai key khác nhau có thể là thiết kế hợp lệ nếu có binding đúng; không sửa bằng cách ép mọi public key bằng nhau.

**Khắc phục đề xuất:** xác định contract và binding từ evidence/vector độc lập; chỉ nhận key pair khi liên kết được chứng thực theo scheme. Reject trước derive/persist/trust mutation; giữ build/runtime gates. Unknown — AMBIGUOUS nếu chưa có evidence Zalo.

**Kiểm chứng:** bundle hợp lệ theo scheme đã chốt; thay DH identity mà giữ SPK/signature/binding cũ phải bị từ chối, trust/ratchet DB không đổi. Test signing key thay thế, invalid signature, missing binding và legitimate distinct signing/DH keys nếu contract cho phép. Kiểm transaction rollback và peer/device scope.

**Tiêu chí đóng:** negative/known-answer tests pass theo contract có provenance. Local hardening có thể hoàn tất trước, nhưng không ghi Zalo interoperability complete nếu thiếu nghiệm thu official hai chiều.

### F14 — P3: Mở diagnostics thực hiện disk/shell trực tiếp trên UI

**Bằng chứng:** `src/gui/tools_panel.rs:1224`; `src/platform.rs:49`. Callback create_dir_all/open_folder đồng bộ và bỏ kết quả.

**Khắc phục đề xuất:** dispatch background theo mẫu hiện có, terminal outcome và status lỗi tiếng Việt; hạn chế request trùng và gắn lifetime thích hợp. Không coi mọi std::thread::spawn trong app là lỗi.

**Kiểm chứng:** mock slow filesystem/shell, UI vẫn nhận input; success/error/missing directory trả đúng completion, không silent success. Native Windows kiểm mở đúng thư mục, Unicode/space paths và shell failure.

**Tiêu chí đóng:** I/O không nằm trong render/input path; kết quả được xử lý, không chỉ toast trước khi tác vụ hoàn tất.

### F15 — P3: Salt dùng LCG seed thời gian/PID

**Bằng chứng:** `src/login.rs:72`–91, caller 188. Trái chuẩn randomness CSPRNG cho salt; chưa chứng minh phá mật khẩu/chiếm account.

**Khắc phục đề xuất:** dùng OS CSPRNG/helper sẵn có, giữ length/alphabet/encoding theo evidence; nếu mapping alphabet thì tránh bias, xử lý RNG error thay vì deterministic fallback. Không thay cipher/signing contract.

**Kiểm chứng:** RNG có thể inject ở test boundary để xác nhận dùng đúng nguồn, length/alphabet và failure propagation. Test request fixture không đổi schema. Không dùng “không trùng trong N lần” làm bằng chứng cryptographic randomness.

**Tiêu chí đóng:** đường production không dùng clock/PID PRNG, RNG failure fail đúng, request compatibility được giữ; native login acceptance khi cần được ghi riêng.

### F16 — P3: Runtime fallback sticker host hardcode ngoài endpoints

**Bằng chứng:** `src/chat.rs:3296`. Đây là fallback runtime, không phải URL fixture.

**Khắc phục đề xuất:** đưa constant sang `src/endpoints.rs`/resolver phù hợp; giữ nguyên host/route/request semantics và service-map precedence. Không biến refactor này thành đổi server contract.

**Kiểm chứng:** fixture dataUrl có/không có, service override và fallback; request target không đổi, shared client/URL validation policy giữ nguyên. Static scan phân biệt production literal và fixtures để tránh false positive.

**Tiêu chí đóng:** nguồn host production duy nhất đúng invariant; fallback/override tests pass và không hardcode mới ở module khác.

### F17 — P3: Prepush kiểm chưa đủ so với nhãn quảng bá

**Bằng chứng:** `scripts/audit-prepush.ps1:45`–58. Step4 chỉ in PRODUCT_VERSION, không kiểm 4-point; stub scan warning-only nhưng banner báo toàn bộ thành công. Cargo/clippy gates thực có kiểm exit; chưa chứng minh release hiện tại mismatch.

**Khắc phục đề xuất:** khi applicable, thực kiểm tag/PRODUCT_VERSION, updater regression expectation, Cargo.toml/Cargo.lock, CHANGELOG. Tách heuristic warnings khỏi hard pass/fail, nói đúng coverage. Không dùng regex scan để chứng nhận toàn bộ UI end-to-end, không tự tạo tag/release khi kiểm.

**Kiểm chứng:** fixture temp repo/files cho đúng version và từng mismatch riêng, thiếu data, branch không tag, cargo/clippy fail. Assert exit code/message chính xác; heuristic hit được ghi là warning/review-needed theo policy rõ, không giả “đã chứng minh UI hoàn chỉnh”. Không commit/push/run release trong test.

**Tiêu chí đóng:** từng mismatch bắt được khi phải kiểm, tool output phân biệt checked/skipped/heuristic; build script release guard hiện có không bị yếu đi.

### F18 — P2: Parser thiếu so sánh sender với UID của chính account

**Bằng chứng:** `src/chat.rs:5958`, `:6127`–6137; caller `src/gui/worker.rs:1130`; merge tại `chat.rs:6465`, append 6531. Có account context nhưng outgoing chỉ dựa 0/rỗng/flags.

**Trigger/tác động:** message mới có uidFrom=self, không flags và chưa có optimistic row có thể nhận is_outgoing=false. Không kết luận mọi mobile-sync đều sai: phone-transfer có adapter khác, echo có thể reconcile. Chưa có capture E1 cho mọi shape như báo cáo ngoài tuyên bố.

**Khắc phục đề xuất:** normalize self UID ở adapter đúng, dùng account UID chứ không CLI profile name, giữ sender identity trong wire reference và precedence flags theo evidence. Không suy enum/schema từ route.

**Kiểm chứng:** parser+merge fixtures self/peer/0, flags bool/int, direct/group, new row/echo, uid aliases đã có evidence. Self input theo contract phải outgoing, peer vẫn incoming; unread/sender-dependent action và duplicate reconciliation không sai. Replay cùng fixture qua WebSocket/poll adapter, kiểm phone transfer độc lập nếu chạm code đó.

**Tiêu chí đóng:** direction tests theo adapter pass; interoperability chỉ được ghi khi có sanitized official fixture/native sync kiểm chứng.

### F19 — P2 experimental: Fingerprint local khác thuật toán Signal được tuyên bố

**Bằng chứng:** `src/crypto/safety.rs:1`–10, `:37`–70; `crypto/session.rs:292`; `gui/worker.rs:1393`. Local hash chung hai sorted raw keys không nhận identifiers. [Signal primary implementation](https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/fingerprint.rs) tạo fingerprint riêng từng bên theo stable identifier/version/serialized key rồi kết hợp.

**Đã kiểm offline:** mô hình chính thuật toán local cho public Signal vector cho `293174380292842574899914978875101211575590763278798213874170`, khác expected `300354477692869396892869876765458257569162576843440918079131`. Đây là Python mirror, không thực thi Rust/native Zalo. Không chứng minh “mọi mã Zalo không bao giờ trùng” hoặc exploit; note cũ đã ghi gap này.

**Khắc phục đề xuất:** chốt scheme/version/identifier/key serialization bằng evidence. Nếu chưa có vector Zalo, gọi đúng local experiment và không quảng bá official verification. Không dùng công thức minUID giả định từ Antigravity, không copy libsignal implementation vào sản phẩm.

**Kiểm chứng:** known-answer độc lập qua hàm Rust thật, test swap hai bên, identifier/key/version thay đổi, serialization prefix và invalid length. Nếu chọn scheme tương thích Signal thì phải khớp vector Signal; để claim Zalo còn cần expected output/evidence chính thức tương ứng, không chỉ Signal pass. Verify state phải owner/peer/device scoped và xử lý identity change đúng.

**Tiêu chí đóng:** doc/UI và hàm thống nhất scheme, Rust known-answer pass; trạng thái parity Zalo không bị nâng từ suy luận. F19 khác F13 binding bundle và F10 assurance.

### F20 — P2: Hai menu tài khoản cùng xuất hiện trong rail và overlay

**Bằng chứng:** `src/gui/workspace.rs:241`–274, `:4274`; `gui/account_switcher.rs:32`, `:61`–85; `gui/modals.rs:18`. Cùng profile_menu_open vẽ inline header/list ở rail 64pt và foreground popover; list expanded có thể cao tới 240px.

**Giới hạn:** duplicate rendering/normal-layout space có bằng chứng nguồn; chưa đo native flicker hoặc chứng minh mọi viewport đều tràn.

**Khắc phục đề xuất:** một owner/anchor/state cho menu account, giữ rail geometry và switch/add command thật. Không xóa chức năng chuyển tài khoản vì nhầm component là dead code.

**Kiểm chứng:** một/nhiều/không profiles; expand/collapse, click outside, Escape, chọn cùng account/khác account, add account. Assert chỉ một menu surface và chỉ một command mỗi action; viewport nhỏ/lớn, DPI 100/150/200%, tên dài. Native screenshot kiểm rail/nav hit targets ổn định.

**Tiêu chí đóng:** command/state tests pass và kiểm layout native; không claim hết flicker chỉ từ unit test.

### F21 — P2: Windows Hello chưa dùng desktop owner HWND, thiếu bounded recovery

**Bằng chứng:** `src/gui/auth_guard.rs:141`–177; `src/gui.rs:5572`–5636, `:3053`–3068. eframe desktop gọi RequestVerificationAsync không có HWND. [Microsoft documentation](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier) yêu cầu desktop HWND + IUserConsentVerifierInterop::RequestVerificationForWindowAsync.

**Trigger/tác động:** sau availability true và auto-lock, API có thể fail/không có owner phù hợp; nếu operation không kết thúc thì auth_in_progress giữ true, nút disable. Có lỗi thì chỉ thử lại Hello trong phiên, thiếu recovery. Không gọi “khóa vĩnh viễn”: guard mới unlocked khi restart; verifier có thể dùng PIN. Chưa chứng minh mọi máy native đều lỗi.

**Khắc phục đề xuất:** interop desktop có HWND/lifetime đúng, typed outcome/deadline và generation để bỏ completion cũ. Timeout không tự hủy được mọi WinRT operation: phải xử lý kết quả muộn và không sinh vô hạn blocking tasks. Recovery/signout/re-auth phải xóa account state đúng, không tạo nút bypass xác thực hoặc local password tùy tiện.

**Kiểm chứng:** mock availability, verified/rejected/cancel/device unavailable/error/Pending; sau deadline UI có đường thử lại/recovery theo thiết kế, completion cũ không unlock session khác hoặc request mới. Native ARM64 kiểm HWND thực, minimize/close window, PIN/face/fingerprint được hỗ trợ, cancel/retry và lỗi thiết bị. Không thử trên phiên thật của người dùng khi chưa có kế hoạch nghiệm thu.

**Tiêu chí đóng:** desktop API ownership được xác nhận, mọi request có state kết thúc hữu hạn, không bypass unlock và không task leak; native acceptance ghi thiết bị/build/kết quả.

## 4. Quy trình kiểm chứng chung và định nghĩa hoàn tất

### Quy trình cho mỗi nhóm sửa sau này

1. Đọc lại source và chuẩn tương ứng vì line numbers ở đây thuộc snapshot đã nêu. Giữ worker ownership, account isolation, signing/cipher flow và chính sách sản phẩm.
2. Tạo regression kiểm failure/observable outcome; nếu có thể, chứng minh thất bại trên baseline rồi pass sau sửa. Không tạo test chỉ mirror implementation. Tests không chạm live Zalo/session/DB người dùng.
3. Sửa nhỏ theo nhóm phụ thuộc; mọi interactive action giữ chuỗi UI→command→backend→payload event→state→render. File/schema/crypto change cần chuẩn và evidence/ADR tương ứng.
4. Chạy focused tests trước, sau đó các gates phù hợp bên dưới. Lưu command, exit code, log, ignored cases và platform, không dùng “tests pass” thay acceptance cụ thể.
5. Đánh dấu riêng code verified, fixture verified và native/official verified. Chưa có evidence thì để còn mở đúng phần, không chuyển toàn finding sang hoàn tất vì compiler xanh.

### Gates dự kiến sau khi có thay đổi code

| Gate | Khi áp dụng |
|---|---|
| `cargo check --locked --all-targets` | Mọi thay đổi Rust/bindings. |
| `cargo fmt --all -- --check` | Kiểm format; khi sửa code mới format theo scope. |
| `cargo test --locked` | Regression suite mặc định sau nhóm sửa. |
| `cargo test --locked --features experimental-e2ee` | E2EE, trust/UI policy, shared message/session paths hoặc thay đổi ảnh hưởng feature build. |
| `cargo clippy --all-targets --locked -- -D warnings` | Gate lint mặc định. |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | Kiểm feature paths sau thay đổi liên quan. |
| `powershell scripts/audit-prepush.ps1` | Trước commit/push khi đã được phép; chưa được coi heuristic là chứng nhận end-to-end. |
| Native Windows ARM64 build/tests/smoke | Các nhóm chạm Win32/audio/Hello/renderer/packaging; release acceptance phải ở target thật. |
| `cargo audit` | Trước release/nâng dependencies; đọc cả informational warnings và target applicability. |

Trên CMake 4.x giữ `CMAKE_POLICY_VERSION_MINIMUM=3.5`. Tests được khởi chạy với data root temp cô lập; loopback chỉ cho synthetic server khi test cần. Store unit tests dùng `:memory:`; reopen/process lifecycle cần fixture temp riêng, không DB thật. Không tự bật workflow/release trong quá trình kiểm.

**Định nghĩa hoàn tất một finding:** root cause được sửa ở boundary thật, regression và gates cần thiết pass, không phá invariant/known behavior, ghi rõ evidence còn thiếu và cập nhật docs/tracking tương ứng. Nếu native/official là phần acceptance bắt buộc thì finding chỉ “code/fixture verified”, chưa “fully accepted”, cho tới khi có biên bản native/official. Đối với crypto không có contract, giữ hạn chế thử nghiệm thay vì tuyên bố parity đã sửa.

## 5. Tối ưu thuật toán và dọn code sau sửa đúng đắn

Các mục dưới là backlog đề xuất, **không cộng thành finding F22 trở đi** và chưa có cam kết tỷ lệ tăng tốc.

| Ứng viên | Hướng xử lý | Kiểm chứng trước/sau |
|---|---|---|
| Audio repaint | F09: bỏ clone theo S samples mỗi frame, chuyển/copy khi có hành động | Allocation/frame, peak RAM, p95/p99 input latency với PCM 1/10/30 phút; output audio không đổi. |
| GUI message tail `workspace.rs:2601` | Reverse take đủ K hoặc cache theo revision thay full O(N) scan mỗi repaint; worst-case filter vẫn O(N) | N=1k/10k/100k synthetic, K cố định; giữ jump/star/call-filter/album order và đo frame/allocation. |
| TUI merge `tui.rs:1340`, `:1373` | Map identity→index mỗi batch thay position lặp O(T×M), hướng O(T+M) | T=100/1k/10k, duplicate/echo/order cases; so kết quả trước/sau và latency. |
| TUI persistence | Delta + single writer sau F03/F04/F06, tránh clone/ghi full snapshot | SQL row writes, queue depth, disk throughput; tombstone/recall/account isolation và crash recovery không đổi. |
| Mentions/style và link extraction | Cache normalized spans theo revision; tính link view một lần mỗi render | Text không style/mentions/link, edited/recalled messages, metadata renewal, owner switch; đo allocation/CPU. Epaint đã có GalleyCache, không mặc định thêm LRU trùng. |
| Avatar mesh | Precompute unit circle/reserve/cache hợp lý nếu profiler chứng minh đáng làm | Ảnh không vuông, UV/crop/DPI/theme; giữ texture và biên tròn. Rounded color rect không là thay thế tương đương avatar. |
| SQLite statements | prepare_cached chọn lọc trên connection sống đủ lâu | Đo parse SQL riêng với DB I/O/DPAPI/crypto; không thay migrations/startup máy móc, không hứa giảm 70%. |

Benchmark phải ghi hardware/OS/profile/dataset/concurrency, warm/cold cache và `task,iterations,min_ms,avg_ms,p50_ms,p95_ms,p99_ms`. So sánh trên cùng điều kiện không chạy compilation nền; số debug audit cũ chỉ tham khảo. Cache key phải chứa scope owner/thread/identity và content/style/font/scale/theme revision cần thiết, có budget/invalidation; không dùng `(u64 msg_id,width,dark)` như bảo đảm đúng.

**Ứng viên dọn nhỏ:** wrapper `ZaloGuiApp::sender_avatar_for` tại `workspace.rs:5052` (giữ free function có caller); private method `ChatMessage::merge_media_metadata` tại `chat.rs:526`; một số suppression dead_code trên field đang được dùng tại `gui.rs:642`, `:870`, `:1044`. Trước xóa phải kiểm mọi cfg/tests/API và compile target liên quan. Chưa xóa mục nào.

**Không xóa theo suy đoán:** range_request là public API có tests; show_incoming_call được cố ý giữ chờ quyết định maintainer; AccountSwitcher/video_state có caller; CallAgain có StartCall handler; DocViewer là viewer text/JSON/log thật, PDF chưa có là scope gap. Không kéo thêm PDF backend hoặc đổi chính sách call log trong đợt dọn code.

## 6. Những nhận định đã bác bỏ hoặc chưa đủ bằng chứng

- Chung file e2ee.db không chứng minh cross-account overwrite: root có profile scope, schema/query có profile_id và manager bound owner. Muốn báo leak phải chỉ ra query/caller sai scope.
- Không giữ kịch bản chắc chắn “trial off⇒Reply plaintext”: peer required đã có plain-send guard. Unknown peer/no-meta cần evidence; structural wire/base64 không phải authenticated policy. Không blanket-lock mọi base64 giống wire.
- Không có bằng chứng crash no-reactor từ entry hiện tại: main là Tokio context và run_native gọi trong context đó. Spawn không tự là blocking I/O UI.
- LocalCallRecord discard phù hợp quyết định ẩn call logs khỏi chat/preview/search/unread trong `docs/REPAIR_VALIDATION_2026-11-03.md:14`. Không tự phục hồi bubbles hoặc xác nhận mọi event mới đã persist từ riêng việc hide.
- LoadOlder đã dispatch LoadGroupHistory; MediaNeighbor xử lý payload; server_pins được reset. Không gắn nhãn stub/leak chỉ vì một đoạn code rời.
- Không giữ P0/P1 hiệu năng, “giảm 70%”, “zero allocation”, “mọi frame 60/120 FPS” khi chưa đo. URL extraction dùng find/split/lines, không regex. Epaint đã có GalleyCache.
- Không giữ chữ Windows Hello “khóa vĩnh viễn” hoặc “không có PIN”; F21 là lỗi desktop integration/recovery có điều kiện. Restart unlocked là workaround hiện hữu, không chứng nhận thiết kế khóa đã hoàn chỉnh.
- Unknown parity Zalo của fingerprint/bundle không được nâng thành exploit hay hợp đồng server. Route index chỉ chứng minh route tồn tại.

## 7. Kết quả kiểm tra đã có và giới hạn

Các số dưới thuộc lượt audit trước trên **cùng HEAD**, không phải kết quả mới chạy trong lượt hợp nhất tài liệu này. Host: Windows 11 Pro 10.0.26300, Intel i5-12400F, x86_64-pc-windows-msvc, rustc 1.99.1, debug.

| Kiểm tra đã chạy | Kết quả được ghi nhận |
|---|---|
| cargo check --locked --all-targets | PASS, exit 0. |
| cargo test --locked | PASS, 856 passed / 0 failed / 13 ignored. |
| cargo test --locked --features experimental-e2ee | PASS, 859 passed / 0 failed / 13 ignored; phần lớn test trùng suite mặc định, không cộng thành số test độc lập. |
| Clippy default và all-features, all-targets, -D warnings | PASS, exit 0 cả hai. |
| cargo fmt --all -- --check | PASS; không format source. |
| cargo audit --json | Exit 0, vulnerabilities.count=0; vẫn có 4 unmaintained + 2 unsound informational warnings. |
| SQLite offline models | F03/upsert còn row và F07/ALTER fail nhưng tăng version đều reproduced; không phải Rust integration. |
| Safety comparison | Python mirror local khác public Signal vector; không chứng nhận native Zalo. |
| Hai benchmark opt-in có sẵn | Upload checkpoint và finite retry PASS; chỉ đo thuật toán có sẵn, không chứng minh các đề xuất mới. |

13 ignored ở mỗi suite không tính là pass; hai benchmark được chạy riêng sau đó, các ignored khác không tự được tính đã chạy. Có linker warning LNK4199 về /DELAYLOAD:mf*.dll không có imports trên x64; không suy lỗi/đúng Media Foundation ARM64 từ warning đó.

Dependency audit dùng DB commit `02e2517f457a359fc6d3440ea4560c8ea188fee8`, cập nhật 15/11/2026, 618 lock dependencies. Unmaintained: audiopus_sys, paste, proc-macro-error, ttf-parser; unsound: glib, rustix. Truy target Windows ARM64 không có glib/rustix1.1.5; cảnh báo lockfile chưa tự chứng minh runtime Windows ảnh hưởng, nhưng vẫn cần theo dõi. Đây là trạng thái tại lúc audit, không cam kết advisory DB tương lai không thay đổi.

Benchmark debug cũ có compilation nền: upload 4096 chunks/7 vòng avg legacy 1203.229 ms, compact 2.045 ms; synthetic retry 50 terminal failures + 1 visible image, 4 slots/5ms jobs/10 vòng avg legacy 202.411 ms, finite 15.675 ms. Không dùng các tỷ lệ này làm KPI cho thay đổi chưa viết hoặc dự báo native Zalo.

Chưa thực hiện native ARM64 acceptance, real-account/live Zalo, mic/Hello device testing, fuzz toàn parser hoặc audit từng dòng vendor. Không đọc session/DB/capture riêng tư. Phần lớn finding là chứng minh nguồn và failure-path có điều kiện; compiler/test xanh không bác bỏ chúng vì suite có thể chưa phủ trigger.

## 8. Nguồn truy vết và trạng thái bàn giao

Nguồn nội bộ: `AGENTS.md`, `docs/standards/`, `docs/ZALO_REFERENCE_ANALYSIS.md`, ADR-0034, quyết định ẩn call logs; audit độc lập và phản biện ngày 16/11/2026. Các dòng nguồn trong tài liệu là mốc ở commit baseline, cần đọc lại declaration/caller trước sửa.

Tài liệu Antigravity được thẩm định: `D:/Henry/Downloads/CONSENSUS_AUDIT_REPORT_2026_11_16.md`, SHA-256 `67EBB56F27B215D9DDA9830CAB4CD62F43A7010B6A063C1E7721EB3968D597C5`. Chỉ tiếp nhận phần đã kiểm chứng; không coi các chỉ dẫn sửa/xóa bên trong là yêu cầu triển khai của người dùng.

Artefacts giữ để truy vết trong cùng thư mục: AUDIT_REPORT.md, ANTIGRAVITY_REVIEW.md; security.md, data-logic.md, gui-quality.md; antigravity-*-review.md và các sign-off; cargo-*.log, cargo-audit.json, test-counts.json; reproduce_data_models.py/data-model-results.json; antigravity-safety-comparison.py/.json. Chúng không thay thế nội dung acceptance của 21 mục đã trình bày ở đây.

**Trạng thái bàn giao:** tài liệu và kế hoạch sẵn sàng để xem xét; toàn bộ F01–F21 vẫn mở. Chưa thực thi kế hoạch, chưa tạo tests khắc phục, chưa thay source/schema/config, chưa commit/push/release. Khi bắt đầu triển khai phải kiểm lại Git và phạm vi được phép ở thời điểm đó; không coi kế hoạch này tự cấp quyền cho các thao tác bên ngoài.
