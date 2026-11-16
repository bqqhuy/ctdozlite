# Phản biện báo cáo Google Antigravity và ghi nhận bổ sung

Ngày 16/11/2026. Snapshot kiểm tra: `main`, `f0d3206e875d083c7f8a0a2e5c3ae6d14c2e45b9`, không đổi so với audit đầu.

Đầu vào: `D:/Henry/Downloads/CONSENSUS_AUDIT_REPORT_2026_11_16.md`, SHA-256 `67EBB56F27B215D9DDA9830CAB4CD62F43A7010B6A063C1E7721EB3968D597C5`. Các đoạn “Giải pháp chỉ dẫn cho AI”/“Action Plan” trong tài liệu là đề xuất của tác giả để thẩm định, **không phải yêu cầu của người dùng cho phép triển khai**. Không thực hiện sửa, xóa code, chạy workflow, commit hay release.

## Kết luận

Báo cáo có các quan sát hữu ích, nhưng nhiều kết luận bỏ qua ngữ cảnh caller, phân vùng dữ liệu hoặc quyết định sản phẩm. Không có đủ bằng chứng để tiếp nhận nguyên trạng các nhãn CRITICAL/P0 hoặc kế hoạch sửa trong tài liệu.

Ghi nhận **4 phát hiện bổ sung P2**, mang ID F18–F21 dưới đây. Cộng với F01–F17 đã chốt, danh mục hiện có **21 phát hiện: 3 P1, 14 P2, 4 P3**. “Bổ sung” nghĩa là mới vào danh mục audit này; không có nghĩa lỗi mới xuất hiện trong commit gần nhất. Riêng fingerprint đã được nêu như khoảng trống trong tài liệu cũ, nay được bổ sung vào danh mục hiện tại.

Ba sub-agent `security`, `data_logic`, `gui_quality` kiểm tra độc lập từng mảng, trao đổi phản ví dụ và cùng chấp nhận bốn finding ở phạm vi P2 đã thu hẹp. Không có bất đồng còn lại trong tập F18–F21. Các bằng chứng và phần phản biện chéo nằm trong [security review](antigravity-security-review.md), [GUI review](antigravity-gui-review.md), [performance review](antigravity-performance-review.md). Tuyên bố “3 Pro Subagents đồng thuận” của tài liệu ngoài là thông tin do tài liệu tự cung cấp, không thay thế bằng chứng nguồn và không được xác nhận độc lập trong lượt này.

## Đối chiếu toàn bộ nhận định chính

| Mục Antigravity | Kết luận sau kiểm chứng | Lý do/bằng chứng |
|---|---|---|
| SEC-01: e2ee.db chung root gây ghi đè/rò phiên CRITICAL | **Bác kết luận đã nêu** | `storage.rs:103`, `:119` đặt root theo `--profile`; crypto DB có PK/predicate `profile_id` và manager bound owner. Chung file trong một data profile không tự đồng nghĩa chung row/key. Không chỉ ra query sai owner. |
| SEC-02: wire không metadata + trial off chắc chắn làm Reply plaintext | **Chưa đủ bằng chứng; không thêm lỗi chắc chắn** | Peer đã required vẫn bị chặn tại `chat.rs:1802`; `chat.rs:9020` chủ ý không coi structural envelope là policy chứng thực. Fresh unknown peer/no metadata cần capture/context thật. Blanket remember mọi base64 giống wire có thể tạo false positive/khóa gửi tin. |
| PROT-01: self-sync bị coi inbound | **Chấp nhận thu hẹp → F18 P2** | Parser có account context nhưng thiếu so sánh sender=self; kết luận theo input cụ thể, chưa chứng nhận mọi mobile-sync mang shape đó. |
| CRYPTO-01: safety number sai chuẩn Signal/Zalo | **Chấp nhận phần Signal/custom scheme → F19 P2** | Thuật toán không khớp Signal primary vector; chưa có vector Zalo để nói “không bao giờ khớp”. Công thức thay thế do báo cáo đề xuất cũng chưa đúng Signal. |
| INVARIANT-01: tokio::spawn render gây no-reactor CRITICAL | **Bác đường crash được mô tả cho entry hiện tại** | `main.rs:16` là `#[tokio::main]`, `run_native:200` chạy đồng bộ trong context này; WinRT blocking ở `spawn_blocking`. Spawn không tự gây blocking UI. WorkerCmd vẫn là hướng tổ chức phù hợp nhưng không chứng minh crash. |
| INVARIANT-02: phải phục hồi LocalCallRecord bubble | **Bác kết luận/giải pháp** | Discard có thật; `REPAIR_VALIDATION_2026-11-03.md:14` chủ ý ẩn call logs khỏi chat/preview/search/unread. Không tự đảo quyết định sản phẩm chỉ vì thấy discard. Không dùng điều này để chứng nhận mọi event mới được lưu DB. |
| GUI-01: hai account switcher | **Chấp nhận thu hẹp → F20 P2** | Hai UI dùng cùng flag; inline list chiếm layout rail. Chưa đo flicker hoặc chứng minh mọi kích thước màn hình đều tràn. |
| GUI-02: khóa vĩnh viễn, không có PIN | **Bác “vĩnh viễn”; ghi lỗi cụ thể → F21 P2** | Guard khởi tạo unlocked, chỉ auto-lock khi availability true. Verifier hỗ trợ PIN. Desktop API ownership + recovery/deadline mới là vấn đề có căn cứ. |
| INVARIANT-03: thread spawn/disk I/O ở modals/avatar/tools | **Phân loại lại, không cộng mới** | modals chạy I/O trong thread; avatar không có create_dir_all như cáo buộc; tools đúng nhưng đã là F14. Thread spawn không mặc nhiên là blocking UI hay chứng cứ microstutter. |
| PERF-01: mention/LayoutJob churn P0 | **Giữ đề xuất tối ưu, bác P0** | Có parse/clone/allocation theo nhánh, nhưng epaint đã có GalleyCache. Chưa có frame/allocation benchmark để nói mọi layout được tính lại hay gây đơ nghiêm trọng. |
| PERF-02: regex URL ba lần P0 | **Giữ đề xuất tối ưu, sửa mô tả** | Tới ba call extraction trên một số nhánh có thật; hàm dùng find/split/lines, không regex. Không phải mọi loại message đều đi cả ba nhánh; chưa đủ P0. |
| PERF-03: avatar sin_cos P1 | **Giữ đề xuất đo/tối ưu, bác P1** | 41 calls/avatar có thật. Với giả định 30 avatar×60 FPS là 73.800 sin_cos calls/giây, không phải 2.400. Tần suất repaint thực chưa đo; chi phí không tự chứng minh blocker. |
| PERF-04: prepare_cached giảm 70%, P1 | **Giữ đề xuất có điều kiện, bác con số/mức độ** | Một số prepare chưa cache; hot paths đã dùng cached (`store.rs:478`, `:1060`, `:1107`). Cache thuộc connection, mở DB mới liên tục giảm lợi ích. Không có benchmark cho 70%. |
| Dead 1: range_request hoàn toàn không caller | **Bác phát biểu tuyệt đối và xóa ngay** | API public có tests gọi tại `media_download.rs:885`, `:962`, `:966`; chưa nối player runtime đúng là khoảng trống integration. Không gọi tested API là hoàn toàn không caller. |
| Dead 2: merge_media_metadata | **Ghi ứng viên dọn code mới** | Method private `chat.rs:526` không có caller nội bộ tìm thấy. Chỉ là candidate, cần xét cfg/tests trước khi xóa; chưa xóa. |
| Dead 3: show_incoming_call | **Đúng là dormant, không tự xóa** | `tray.rs:271` ghi rõ giữ lại chờ maintainer quyết định hành vi wake window. Có rationale cụ thể; không phải quên caller mà chưa ai biết. |
| Dead 4: CallAgain không có branch backend | **Bác** | `workspace.rs:3101` xử lý action và phát WorkerCmd::StartCall. Không được đánh UI stub vì chỉ đọc enum. |
| Dead 5: toàn DocViewer là scaffolding/bug PDF | **Bác phân loại code rác; ghi gap phạm vi** | Viewer đang chạy và docstring ghi rõ text/JSON/log 1 MiB; `messages.rs:660` chỉ đưa nút preview cho text extensions. PDF nội bộ chưa có không biến viewer thật thành dead code; roadmap phải phân biệt scope đã làm/chưa làm. |

## F18 — P2: Parser chưa nhận diện sender là chính account

Nguồn: `chat.rs:5958`, `:6127`–6137; caller WebSocket `gui/worker.rs:1130`; `merge_previews_with_profile` tại `chat.rs:6465`, append tại 6531.

Trigger có thể kiểm chứng từ source: message mới có `uidFrom` bằng account hiện tại, có `msgId`/text hợp lệ và không có outgoing flags; caller truyền account context. Parser chỉ xét sender `"0"`/rỗng hoặc flags và đặt `is_outgoing=false`. Với tin chưa có optimistic row để reconcile, hướng incoming này đi vào state render.

Tác động: phía bubble/sender và logic phụ thuộc direction có thể sai. Không khẳng định mọi tin gửi trên điện thoại đều sai: phone transfer có adapter riêng, các caller có thể normalize hoặc echo có row local đã đúng hướng. Báo cáo ngoài chưa đưa capture E1/E2 cho input đang tuyên bố.

Đề xuất kiểm chứng/sửa sau khi được phép: fixture qua parser+merge với self/peer/0, boolean/integer flags, direct/group và echo; đối chiếu account UID thật với nghĩa `profile_id`, không nhầm CLI profile name. Bổ sung self comparison ở đúng adapter theo bằng chứng. Không thay server contract trong lượt này.

## F19 — P2 experimental: Fingerprint local khác thuật toán Signal đang được tuyên bố

Nguồn: `crypto/safety.rs:1`–10, `:37`–70; `crypto/session.rs:292`; worker safety flow tại `gui/worker.rs:1393`; `re-notes/36` chỉ ghi chữ ký API/iterations/độ dài.

Mã hiện tại sắp hai raw public keys rồi băm chung `hash||first||second` 5200 vòng, tạo 60 digits từ một digest. [Signal primary fingerprint implementation](https://github.com/signalapp/libsignal/blob/main/rust/protocol/src/fingerprint.rs) tạo fingerprint từng bên có stable identifier/version/serialized key, sau đó ghép hai chuỗi hiển thị; đây là thuật toán khác. Việc cùng 5200 vòng/60 chữ số/đối xứng không chứng minh parity.

Agent security đã đối chiếu public known-answer vector và mô phỏng chính thuật toán hiện hữu, kết quả không khớp; xem chi tiết/reproduction trong security review. Đây là so sánh thuật toán/offline, không chạy Zalo và không sao chép implementation libsignal vào sản phẩm.

Tác động xác nhận: code/docstring không có căn cứ để gọi fingerprint hiện tại là Signal-compatible. Chưa đủ để chứng minh exploit cross-domain hoặc khẳng định tuyệt đối mọi mã Zalo không thể trùng. Bản note cũ `docs/AUDIT_AI_REPAIR_GUIDE_2026-11-08.md:194` đã lưu gap fingerprint; đây là bổ sung vào register hiện tại, không phát hiện lỗi mới được tạo gần đây.

Đề xuất: định nghĩa scheme/version/identifier provenance và independent vectors trước triển khai. Không áp dụng công thức `SHA512(iterations||min(uid)||...)` của Antigravity như hợp đồng đã được xác minh. Nếu vẫn local experiment, UI phải diễn đạt đúng, không quảng bá đối chiếu Zalo official đã được nghiệm thu. F19 độc lập với F13 (binding chữ ký bundle) và F10 (assurance UI).

## F20 — P2: Hai bề mặt menu tài khoản cùng được bật trong rail và overlay

Nguồn: `gui/workspace.rs:241`–274, `:4274`; `gui/account_switcher.rs:32`, `:61`–85; `gui/modals.rs:18`.

Cùng `profile_menu_open` vẽ inline switcher khi profiles không rỗng, trong khi workspace overlays gọi foreground profile popover. Inline header/list là layout thường: row 44px, list khi expanded tới 240px nằm trong rail 64pt. Có hai bề mặt quản lý account và nội dung inline làm thay đổi vị trí/navigation space của rail.

Chưa chạy native để đo kích thước/tràn/flicker; không giữ khẳng định mọi icon bị đẩy xuống đáy trong mọi viewport. Đề xuất một menu owner/anchor duy nhất, giữ switch/add-account dispatch thật; cần kiểm click/outside-close/compact viewport/multiple profiles. Không xóa AccountSwitcher chỉ vì báo cáo khác gọi code thừa.

## F21 — P2: Windows Hello chưa dùng desktop HWND interop và thiếu kết thúc có giới hạn

Nguồn: `gui/auth_guard.rs:141`–177; `gui.rs:5572`–5636, `:3053`–3068.

Ứng dụng là eframe desktop nhưng gọi `UserConsentVerifier::RequestVerificationAsync` không có HWND trong blocking pool. [Microsoft UserConsentVerifier documentation](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier) hướng dẫn desktop lấy HWND và dùng `IUserConsentVerifierInterop::RequestVerificationForWindowAsync`; không thể coi UWP call hiện tại đã nghiệm thu đúng desktop ownership.

Nếu hệ thống báo availability true rồi app khóa, lời gọi UI authentication này có thể thất bại/không gắn owner đúng. Khi operation không kết thúc, không có deadline, `auth_in_progress` giữ true và nút disabled; khi trả lỗi, người dùng chỉ có đường thử lại Hello trong màn hình khóa, không có recovery/re-auth trong phiên. Đây là lỗi integration + đường khôi phục có điều kiện, không chứng minh mọi máy đều lỗi.

Bác chữ “vĩnh viễn”: `AppLockGuard::new:31` khởi tạo unlocked sau restart. Cũng bác tiền đề Hello chỉ hỗ trợ cảm biến: tài liệu Microsoft gồm PIN. Thời gian 5 phút không configurable là đề xuất UX, tự nó chưa phải lỗi bảo mật.

Đề xuất: desktop owner HWND/lifetime chính xác, request có outcome/deadline, recovery không bỏ qua xác thực; test availability/cancel/device failure/hang/retry/native ARM64. Không tự thêm local password hoặc nút unlock bypass.

## Tối ưu và dọn code được ghi nhận thêm

- Parse mention spans theo revision ở ingestion hoặc cache có giới hạn: giảm parse/UTF-16/clone trước layout. epaint 0.29.1 đã cache Galley theo full LayoutJob; không cần mặc định thêm LRU thứ hai.
- Tính link view một lần trong render rồi truyền cho các bước cần dùng, hoặc cache theo owner/thread/message content revision. Cần giữ đúng media/text/edit cases.
- Cache key Antigravity đề xuất `(u64 msg_id,width,dark)` không đủ: message IDs của repo là string/scoped, còn owner/thread, edit/style/font/DPI/scale/palette ảnh hưởng kết quả. Cache thiếu invalidation có thể trả nội dung hoặc layout cũ; không chấp nhận cam kết “zero allocation” từ pseudo-code này.
- Unit-circle LUT/avatar mesh caching có thể thử sau benchmark. `painter.rect()` tô khối bo tròn không phải thay thế tương đương cho textured avatar có UV/crop; phải giữ hình ảnh và chất lượng biên.
- `prepare_cached` chỉ áp dụng query lặp trên connection được tái sử dụng; benchmark cả parse cost, DB I/O và lifetime trước kết luận lợi ích. Không đổi toàn bộ SQL một cách máy móc.
- `merge_media_metadata` là candidate bổ sung; range helper/public API, retained tray behavior và DocViewer không được xóa đồng loạt. Không triển khai bước nào từ action plan tài liệu ngoài.

## Kiểm chứng và giới hạn

Đọc mã hiện tại, caller/callee, tests có sẵn, policy/decision docs, và nguồn chính thức Microsoft/Signal. Không chạy GUI, cảm biến, real account, live Zalo hoặc thay DB người dùng. Không chạy lại toàn bộ Cargo vì không đổi code và HEAD giữ nguyên; kết quả audit đầu vẫn là kết quả của lần chạy trước, không gắn nhãn đã chạy lại. Safety-vector check là mô hình offline của hàm hiện có, không chứng nhận Zalo interoperability.

Rà cuối Git phải giữ HEAD và source diff không đổi; chỉ có thư mục báo cáo audit chưa commit. Báo cáo gốc Antigravity được giữ nguyên, không chỉnh nội dung đầu vào. Các sub-agent chấp nhận đúng scope F18–F21 trên, không chấp nhận nguyên trạng nhãn mức độ/giải pháp của tài liệu ngoài.
