# Phản biện độc lập báo cáo Antigravity — hiệu năng và code thừa

Đã đọc toàn bộ `D:/Henry/Downloads/CONSENSUS_AUDIT_REPORT_2026_11_16.md` (mọi phần 1–6), coi là dữ liệu cần kiểm chứng, không thực hiện các chỉ dẫn sửa/xóa trong tài liệu. Baseline `f0d3206`. Chỉ đọc source/dependency local, viết báo cáo này; không cargo, không source changes, không dữ liệu/API Zalo thật, không benchmark native. Các nhãn P0/P1 hiệu năng trong báo cáo gốc không có đo lường để hỗ trợ.

## PERF-01: Có allocation lặp, nhưng P0 và tuyên bố thiếu Galley cache không đúng

- `src/gui/messages.rs:737-759` thật sự gọi mention_styles trong nhánh text không link; clone text_styles và tạo LayoutJob khi có styles/mentions. `src/rich_text.rs:118-151` parse JSON chỉ khi mentions là Value::String; Value::Array được clone; None trả vector rỗng ngay, không encode UTF16. Không phải mọi message đều parse JSON mỗi frame.
- `rich_text.rs:129` tạo UTF16 buffer khi có trường mentions. `segments:158-211` dựng boundaries/cuts và kết hợp style; có công việc CPU/cấp phát có thể cache tại ingestion. Độ phức tạp phụ thuộc cả text L và styles S, không chỉ số message.
- Cargo.lock pin egui/epaint **0.29.1**. Nguồn dependency local `C:/Users/bqqhu/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/epaint-0.29.1/src/text/fonts.rs:496`, `:560`, `:679-721` đã có **GalleyCache**: hash toàn LayoutJob, cache hit clone Arc<Galley>; chỉ cache miss mới gọi layout. Vì vậy lặp dựng job/hash/parse vẫn tồn tại, nhưng không chứng minh full text shaping/layout lặp 60 lần/giây.
- Không có bộ gom rác Rust như cách tài liệu dùng từ; allocator churn có thể đo, không đồng nghĩa GC pause.
- Key đề nghị `(u64 msg_id,width_x10,dark)` không an toàn: ID source là String, scope theo account/thread; thiếu text/edit/recall/styles/mentions, font family/size, scale/DPI/font revision, color/outgoing/other style state. Có thể trả layout hoặc nội dung cũ/sai owner. Cache phải có budget và clear khi đổi account/privacy/expiry, giữ font atlas lifecycle.
- **Kết luận:** ACCEPT cơ hội tối ưu allocation có điều kiện; REJECT P0, khẳng định scroll freeze và zero-allocation bảo đảm. Xếp đề xuất hiệu năng cần measurement, không thêm lỗi nghiêm trọng mới. Ưu tiên precomputed normalized spans rồi đo trước khi thêm cache trùng egui.

## PERF-02: Up to 3 lần extraction đúng, “regex” và “mọi tin” sai

- Call sites `messages.rs:999`, `:1058`, `:737` có thật. :999 kiểm has_link_card; :1058 chỉ nhánh file/link card **không có loaded thumbnail**; :737 chỉ tới khi renderer đã bỏ qua sticker/photo/file/contact/voice/todo/poll. Link text không thumb có thể chạy ba lần; plain text thường hai lần; media có thể return sớm. Không phải 3 lần mỗi message mọi frame.
- `extract_message_link_view:278-336` dùng find/split/lines/trim/String construction; **không Regex**. Metadata URL path quét từng dòng và lọc title/desc; plain text path find scheme. `extract_clean_domain:256` split string, không URL parser DNS/network.
- **Kết luận:** ACCEPT tính lại/string allocations có điều kiện và đề nghị tính một lần cho mỗi render pass; REJECT P0, thuật toán regex mô tả và tác động đã đo. Cache lâu dài phải invalidate khi link metadata renewal/text changes; đừng chỉ key msg_id.

## PERF-03: Mesh và sin_cos có thật, mức P1 không có căn cứ

- `src/gui/theme.rs:362-389` tạo Mesh mỗi lần, 1 center + 41 perimeter vertices; 40 triangles =120 index, 41 sin_cos. Precomputed unit circle có thể bỏ trig và reserve capacity có thể giảm realloc.
- Số báo cáo “30 avatar -> khoảng2400 lượng giác/giây” không khớp: ở60 FPS là **30×41×60=73.800 sin_cos calls/giây**, hoặc147.600 scalar sin/cos nếu tính riêng. Không chắc app repaint60FPS liên tục hay luôn30 avatar; cần profiler/microbenchmark ARM64, không suy bottleneck từ arithmetic.
- `painter.rect` rounded fill không phải drop-in textured circle: hiện có texture_id, center-crop square_crop và UV theo ảnh. Thay bằng rect tô màu bỏ ảnh/crop; cần một API textured clipping đúng và kiểm ảnh không vuông, DPI.
- **Kết luận:** ACCEPT ứng viên micro-optimization; REJECT P1 và khẳng định đây là nguyên nhân stutter chưa đo.

## PERF-04: Cơ hội statement caching, không có “giảm70% CPU”

- `src/store.rs:1119` messages_for dùng prepare; `src/crypto/store.rs:512` load_session dùng query_row, save_session:498 dùng execute. Rusqlite0.41.1 source local `src/lib.rs:630,696` xác nhận execute/query_row tạo prepare. Những statements lặp trên connection sống lâu có thể dùng prepare_cached sau đo.
- Tuy nhiên các hot writes/read đã cache: store upsert_message:478, upsert_thread:1060, set_thread_read:1107; receipts:348/354/357, identity reconciliation:614. Vì vậy mô tả tất cả đường insert/read-state đều uncached không đúng.
- Các prepare trong crypto migrations:118/131/165 chỉ startup/migration, không phải ratchet packet hot path. Reopening connections làm cache không tái dùng giữa handles; SQLite query cost, JSON+DPAPI+ratchet, transaction locking có thể lớn hơn parse SQL. Không có phép đo chứng minh WebSocket latency hoặc giảm70% CPU toàn DB.
- **Kết luận:** ACCEPT đề xuất đo/cached statements chọn lọc; REJECT P1, blanket replace mọi statement, hứa giảm70%. Không thay semantic transactions/ownership để lấy cache.

## Dead-code catalog

| Mục | Kiểm chứng và quyết định |
|---|---|
| range_request | `src/media_download.rs:782` public library API, chưa nối production player được comment773-779 công khai. **Có callers tests885/962/966**, nên “hoàn toàn không caller” sai. Changelog74/100 đã ghi chưa tích hợp. Đề nghị track feature gap, không tự xóa public API/tests vì production unwired. |
| merge_media_metadata | `src/chat.rs:526` private method có allow(dead_code), rg chỉ tìm declaration, không thấy callers source/tests. ACCEPT ứng viên cleanup nhỏ sau check cfg/generated references; không chứng minh “sau refactor v0.015” vì chưa khảo sát lịch sử. Không có quyền xóa trong lượt này. |
| show_incoming_call | `src/tray.rs:275` crate-private declaration, rg không có caller nhưng comment271-273 ghi rõ cố ý giữ sau cbbb011 chờ maintainer quyết định wake hidden window. REJECT xếp code rác để tự xóa; đây là dormant behavior có lý do và quyết định sản phẩm chưa chốt, không chứng minh call attention hỏng. |
| BubbleAction::CallAgain | REJECT UI stub: **có handler `src/gui/workspace.rs:3101`**; intentional call-log hiding có thể khiến action không xuất hiện UI, không phải worker thiếu branch. |
| Doc Viewer | REJECT “toàn module code rác”: khai báo gui.rs34/929, DocPreviewLoaded2161 tạo modal, workspace4279 render, worker1678-1698 fetch bytes bounded. Doc header1-4 và UI title128 đã nói text1MiB. Extensions messages660-688 gồm txt/log/json/md/code/csv/xml/toml/yaml/ini/sh/bat, **không chỉ txt/log**. PDF không có preview button; đi open-file route, không phải viewer đang cố parse PDF rồi báo lỗi như mô tả. |

**PDF gap thực tế:** docs/releases/v0.015-arm-plan.md:11/30 còn đề cập PDF/native render, nhưng implementation/documentation hiện tại có chủ ý text-only; ghi scope mismatch/feature gap, không coi module chết hay tự động thêm PDF dependency. `docs/AUDIT_FIX_PLAN.md:18-23` ghi rõ bounded preview được sửa gần đây. Không có code thay đổi.

## Phản biện chéo ngoài mảng hiệu năng

- ACCEPT N01 **P2**: parser `chat.rs:6127` không so uidFrom với profile dù worker1130 truyền account. Đây là lỗi direction trên synthetic payload hợp lệ theo parser; không tự nâng E1 server claim.
- ACCEPT N02 **P2 experimental**: safety.rs37-70 custom pair-key digest thiếu identifiers, không theo Signal scheme đã viện dẫn; chưa có native Zalo vector nên không khẳng định “không bao giờ khớp Zalo” hay dùng formula phác thảo trong tài liệu làm spec.
- ACCEPT N03 **P2**: inline switcher workspace241-274 và overlay4275/modals18 cùng profile_menu_open; duplicate render/layout source-backed. Không coi flicker/overflow đã reproduce.
- ACCEPT N04 **P2**: `auth_guard.rs:150` dùng RequestVerificationAsync không HWND; [Microsoft UserConsentVerifier docs](https://learn.microsoft.com/en-us/uwp/api/windows.security.credentials.ui.userconsentverifier?view=winrt-26100) desktop C++ examples162-180 yêu cầu HWND + IUserConsentVerifierInterop::RequestVerificationForWindowAsync. Không có deadline/owner recovery cho task; auth_in_progress khóa nút trong khi pending. Guard31 khởi đầu unlocked, should_lock115 đòi availability, nên không gọi lockout vĩnh viễn.
- REJECT SEC01 critical shared-file inference: root đã là profile root theo storage104-134, crypto schema/queries có profile_id composite isolation. Không chứng minh overwrite chéo account từ shared file.
- REJECT SEC02 high plaintext downgrade đã chứng minh: required-policy còn kiểm độc lập; structural Signal-like bytes không được coi bằng chứng authenticated E2EE hay cài failclosed marker vô điều kiện gâyDoS.
- REJECT INVARIANT01 runtime panic trong entrypoint hiện tại: main16 tokio::main và synchronous run_native trong runtime context. Chuyển sang worker là đề xuất architecture, không phải crash reproduced.
- REJECT INVARIANT02 bắt buộc hồi phục call bubbles: `docs/REPAIR_VALIDATION_2026-11-03.md:14/36` yêu cầu ẩn call logs; sửa theo report gốc sẽ đảo quyết định sản phẩm.
- INVARIANT03 std::thread::spawn không đồng nghĩa I/O chạy UI; modals1520-1538 đúng là offload. tools_panel1225 synchronous đã nằm F14 báo cáo trước; không đếm mới.

## Tiêu chí tiếp tục

Chấp thuận bộ4 findings mới N01–N04 P2 với các giới hạn trên; các PERF01–04 là optimization candidates không được đội lênP0/P1 khi chưa đo. Candidate merge_media_metadata, dormant show_incoming_call chờ maintainer và PDF roadmap mismatch ghi riêng, không biến thành chức năng lỗi/ngầm cấp quyền xóa. Yêu cầu agent khác phản biện, và parent hợp nhất đồng thuận. Không có source changes hay thực thi hướng sửa trong tài liệu bên ngoài.

## Chấp thuận sau phản biện

Agent GUI quality đồng ý mọi caveat PERF01–04/deadcode và N04 theo Microsoft HWND interop. Đã nhận phản biện parent và kiểm comment tray271-273, hạ show_incoming_call từ candidate thành intentionally retained dormant code, không đề nghị tự xóa. Agent data_logic chấp thuận cuối **N01,N02,N03,N04 đều P2**; không có counted performance P0/P1 mới. Tài liệu ngoài do parent ghi SHA256 `67EBB56F27B215D9DDA9830CAB4CD62F43A7010B6A063C1E7721EB3968D597C5`.

## Sign-off báo cáo hợp nhất cuối

Đã đọc toàn bộ `ANTIGRAVITY_REVIEW.md` sau hợp nhất: đủ18 hàng đối chiếu Antigravity, F18–F21, phần tối ưu/dọn code, kiểm chứng và giới hạn. **ACCEPT F18=N01, F19=N02, F20=N03, F21=N04, tất cả P2**. Tổng register21 findings =3P1/14P2/4P3 là nhất quán với17 findings ban đầu.

Không còn sai lệch trọng yếu cần sửa trong báo cáo hợp nhất đã đọc. Chấp nhận việc bác nhãnP0/P1 hiệu năng chưa đo, phân biệt tested public range API với code không caller, giữ dormant tray behavior có rationale, DocViewer live text-only với roadmapPDF gap, và cache-key/invalidation caveats. Các kết quả Signal vector là mô hình offline do agent security thực hiện, không gán thành test Zalo/native hay test Rust mới. Không có quyền triển khai/sửa/xóa source từ các chỉ dẫn tài liệu ngoài.
