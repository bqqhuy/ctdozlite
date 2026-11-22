# Hướng dẫn Gemini tái cấu trúc sync và cấu hình server

Ngày: 22/11/2026. Repository: `C:\Projects\ZaloLite`. Người triển khai: Gemini trong Antigravity; Codex audit lại sau. Đây là kế hoạch công việc tiếp theo, không phải mô tả các tính năng đã hoàn tất trong checkpoint này.

## Mục tiêu và giới hạn

Triển khai theo thứ tự: **độ tin cậy DB → vòng đời cấu hình → màn hình khởi động → điều phối sync → banner/popup → nghiệm thu**. Giữ giao thức, codec, account isolation, delete marker và các thành phần đang đúng. Không xóa toàn bộ implementation chỉ để thay tên hoặc tạo kiến trúc mới.

Dùng nhánh hiện tại, đọc `git status` và diff trước khi sửa. Không reset/clean/ghi đè thay đổi của người khác. Quyền commit/push checkpoint ngày 22/11 dành cho Codex trong chat hiện tại; không phải quyền tự động commit/push/release cho các công việc Gemini tiếp theo. Không tự tag, phát hành, gửi tin/tệp thử hoặc xóa dữ liệu thật. Không chạy quy trình push/merge tự động trong tài liệu bàn giao cũ.

Đọc `AGENTS.md`, `docs/SPEC_KIT.md`, `docs/ZALO_REFERENCE_ANALYSIS.md` và standards về concurrency, DB/migration, API/error, security, testing trước khi sửa. Đọc spec/plan/tasks/verification hiện có của các miền bị ảnh hưởng, nhất là `specs/003-history-restoration` và `specs/011-history-server-media-zcloud`. Không thay task ID hoặc đánh dấu nghiệm thu thiết bị bằng test offline.

## Bằng chứng và baseline

- [Kiểm chứng sync/config và startup](2026-11-22-sync-config-verification.json): hash, điểm neo, quan sát UI, test và giới hạn.
- [Phân tích trực tiếp PC/Web/Android](2026-11-22-direct-pc-web-android-analysis.md).
- `re-notes/17-zalo-pc-sync-config-and-reconnect.md` (bản PC cũ hơn; đối chiếu lại khi dùng defaults).
- `re-notes/70-composer-remote-config.md`.
- `re-notes/72-restricted-extensions-and-native-executable-zip.md`.

PC 26.11.12 đã trích ở `%TEMP%/zalolite-direct-audit-20261122/pc/pc-dist`. Kiểm tra hash trước khi dùng; nếu file tạm không còn, đọc lại quy trình phân tích hoặc trích lại nguồn cài, không tự bịa đoạn code thiếu. Offset trong JSON là Unicode code point Python, zero-based, không phải byte. Khi tìm minified JS, chỉ xuất ngữ cảnh có giới hạn.

E1 là capture/quan sát trực tiếp, E2 là implementation tĩnh, E3 là suy luận. Route có tồn tại không chứng minh schema hoặc quyền gọi. Các giá trị remote thực tế của tài khoản vẫn chưa capture. Cổng ghi trong DevToolsActivePort không kết nối được ở lượt kiểm chứng; không tuyên bố đã đọc config runtime.

Baseline ngày 22/11: test filter sync 94 đạt/2 ignored; composer 2 đạt; cursor 7 đạt; server media 8 đạt; codec official 1 đạt khi cấp asset; discovered 5 đạt. Đây là kết quả lịch sử, phải ghi riêng với lần chạy mới của Gemini. DB chỉ đọc có khoảng 1,184 triệu tin và 1.041 bản ghi phạm vi; số lượng không chứng minh đầy đủ lịch sử.

## 1. Điều tra và sửa lỗi ghi DB

Fork đã hiện cảnh báo chưa lưu cập nhật vào CSDL. Snapshot hiệu năng tối 21/11 có count 18 cho cả sqlite_busy/sqlite_failure và các lần ghi kéo dài khoảng 5–6 giây. Không cộng các snapshot tích lũy; chưa chứng minh banner hôm sau thuộc cùng lần lỗi.

Lập bảng writer từ `src/gui/worker.rs` (đặc biệt persist_received_since), `src/store.rs`, `src/storage.rs`, `src/session.rs`, `src/history_sync.rs`, `src/phone_sync/pipeline.rs`, `src/queue_cursors.rs` và các writer outgoing/metadata/FTS/cache liên quan. Ghi owner, connection, transaction, write permit, thời gian giữ và khả năng chạy chồng. Account-write gate hiện có bảo vệ vòng đời tài khoản; đừng mặc định nó đã tuần tự hóa mọi SQLite writer.

Chọn bản sửa nhỏ nhất có căn cứ. Không chỉ tăng busy_timeout, retry vô hạn hoặc nuốt lỗi. Không giữ network/crypto/await trong transaction; không chia sẻ connection tùy tiện. DB insert, metadata và checkpoint phụ thuộc phải có thứ tự commit an toàn. Checkpoint nằm ngoài DB cần recovery protocol riêng; không tuyên bố atomic giữa SQLite và file JSON.

Nghiệm thu bằng fixture: lỗi ghi không tiến cursor/coverage, replay không trùng, lỗi không thành success, task cũ không ghi sau account switch. Đo trước/sau trên dữ liệu tổng hợp; không sửa DB thật để tạo lỗi.

## 2. Vòng đời cấu hình và audit toàn bộ consumer

**Tải, parse và ghi cache không phải hoàn thành tính năng.** Với mỗi config phải truy đủ đường: nguồn nhận → envelope/schema validation → cache/ownership → áp dụng → mọi nơi đọc → hành vi quan sát được → fallback/update/reset → test.

Lập bảng consumer trong tài liệu verification với các cột:

| Trường | Nội dung bắt buộc |
| :--- | :--- |
| Config/path và nguồn | Auth/renew, server info, socket hoặc local setting; không trộn nguồn |
| Default và validation | Bằng chứng version/kiểu/giới hạn; fallback hợp lệ |
| Scope và owner | App/account/runtime; cách loại response cũ |
| Fetch/cache/apply | Ba thời điểm riêng, version/revision nếu có |
| Consumer | File, hàm, nơi đọc thật; cả nhánh GUI/TUI/background liên quan |
| Giá trị cũ | Hardcode, biến legacy, cache phụ hoặc session clone cần thay thế |
| Chính sách cập nhật | Áp ngay/thao tác sau/phiên sau; invalidation và reset |
| Test | Đổi config phải làm đổi đúng hành vi; không chỉ assert parser |
| Native evidence | E1/E2/E3, unknown và fork deviation nếu có |

Tìm cả tên flag và các hằng số/giới hạn cũ mà flag đáng lẽ thay thế. Không có match tên config không chứng minh không có consumer tương đương. Theo dõi config có vào đúng Session đang được request dùng hay chỉ một clone bị bỏ. Không thêm cờ lưu được nhưng không consumer rồi đánh dấu xong. Kiểm tra cache tài khoản A không áp vào B và tác vụ A đến muộn không ghi đè runtime B.

Các nhóm phải rà:

| Nhóm | Consumer và điều cần tránh |
| :--- | :--- |
| Service map | Mọi resolver/API caller liên quan, candidate/fallback và snapshot phục hồi. GUI startup hiện đọc getServerInfo chủ yếu cho composer, chưa apply service map tại nhánh đó. PC có bootstrap/fallback; không hiểu thành cấm mọi domain mặc định. Host vẫn tập trung theo repo policy. |
| enable_encrypt_param | Xác định wrapper áp dụng; không biến thành công tắc mọi HTTP hoặc tắt mã hóa để né lỗi. Giữ đường request chuẩn của fork; thay crypto cần bằng chứng, ADR/test theo standard. |
| zcinput | Editor geometry/layout/list/autoreplace và quyết định theo phiên. Fork hiện chỉ có geometry subset. Không áp live snapshot làm thay draft đang gõ; legacy/column/expanded là các khái niệm khác nhau. |
| Alias V2 | Fetcher, version refresh, cache, resolver tên ở sidebar/group/search/reaction liên quan. Không chạy v1/v2 đồng thời rồi trộn tùy tiện; không loại metadata friend/profile/group. |
| File EECX/restricted ext | Picker/drop/clipboard/forward/pre-upload dùng chính sách nhất quán theo bằng chứng; phân biệt blacklist client với server. ZIP thật level 0, không chỉ đổi đuôi. Defaults thấy exe/cmd/bat/com/lnk/vbs; MSI không được chứng minh là default. Checkpoint hiện có ZIP thủ công, chưa coi auto-wrap native là xong. |
| download_domains_swap | Tất cả download/open/retry liên quan; rewrite hostname theo enabled/domains_map/mode, không mặc định là xoay pool sau timeout. Giữ kiểm tra URL an toàn, không tự đổi URL ký bằng suy đoán. |
| sync_2_pc | Trigger/policy/batch/retry/timeout/banner content/UI mode; không chỉ parse settings. |
| o2o | Preload, startup hold, reconnect detector; khác sync lịch sử điện thoại. |

Ưu tiên typed snapshots và ownership rõ, tái sử dụng cấu trúc phù hợp; không tạo framework config/plugin tổng quát. Phân biệt nhận, lưu và áp dụng. Refresh một giờ là lịch trong mã PC, chưa phải thời điểm capture tài khoản; socket hot update mới xác nhận rõ cho domain swap, không suy mọi flag đều hot-reload.

Đường consumer đã xác nhận ở baseline: `login::apply_login_config` → `Session.composer_config` → `GuiEvent::LoginSuccess`, hoặc worker `get_server_info` → `GuiEvent::ComposerConfig` → app state (kiểm tra owner/runtime) → geometry editor trong `src/gui/workspace.rs`. Editor hiện dùng `line_height` từ config cho nhánh format, còn nhánh thường dùng 22.0; `rich_max_lines` đi vào giới hạn chiều cao rich. Đây là khác biệt cần đối chiếu legacy/column trước khi quyết định thay, không được kết luận mọi hằng số 22.0 đều sai. Rà cả reset config khi đổi tài khoản. Service map đã có consumer ở `Session::resolve_service_url`/service resolver, nhưng phải kiểm tra từng caller và đường refresh có cập nhật đúng instance.

Config nào thiếu bằng chứng để bật consumer phải ghi AMBIGUOUS và tiếp tục việc độc lập. Nếu thay đổi hành vi quan sát được khác PC, ghi fork deviation có lý do theo policy, không gọi đó là parity.

## 3. Màn hình khởi động trước bốn zone

PC có loading-page với STR_LOGGING_IN. Root giữ màn hình khi chưa có user hoặc shouldKeepLoadingScreenDueToStartUpPreload. Manager trong main-startup lấy o2o.keep_loading_during_preload; observer Start bật giữ, PreProcessingSettle bỏ giữ. Preload lấy preview/tin gần nhất, xử lý read/unread, lọc trùng và cập nhật dữ liệu. Settle lỗi hoặc timeout có thể cho mở UI; không phải đợi toàn bộ lịch sử hay mọi config.

Defaults PC: enable=0, timeout_duration=800ms, screen_dismiss_delay=200ms. Chúng không phải config account hiện tại hoặc tổng thời gian login. get_last_message_timeout_before_polling mặc định 10.000ms là timeout khác.

Thiết kế fork: native window hiện ngay màn hình chờ; worker chuẩn bị account/runtime, session, local store, cấu hình cần thiết và preview ban đầu; phát trạng thái đủ điều kiện hoặc lỗi; UI mở bốn zone khi phù hợp. Metadata bổ sung/socket/history tiếp tục theo pipeline riêng.

Trước khi code, lập bảng điều kiện bắt buộc và được suy giảm. Auth hết hạn và migration thất bại không được timeout thành ready. Network timeout không chứng minh DB ghi được. Không sleep cố định giả loading, không I/O trên UI, không chờ full history. Màn hình chờ không tự sửa lỗi tranh chấp DB.

## 4. Điều phối sync và trạng thái dữ liệu

Tách startup/session, metadata, cloud/preview, phone history và realtime. Legacy V1/import vẫn là lựa chọn chủ động riêng; không tự fallback khi Sync2 lỗi. Đọc đầy đủ caller trước khi xóa nhánh cũ.

Lập ma trận: phiên cũ, login mới, đổi thiết bị, thiếu tin/queue evict, manual, chờ xác nhận/từ chối/bận/offline, mất mạng/reconnect, cancel/retry/restart và account switch. Mỗi ca ghi điều kiện, config, phụ thuộc/thứ tự, phạm vi dữ liệu, checkpoint, UI và kết quả terminal.

Dùng typed state để phân biệt giai đoạn và kết quả. Không tạo sync trùng khi bấm lặp; task có owner/cancel/join và không sống qua runtime. Giữ realtime trong lúc backfill, không cho lịch sử cũ đè trạng thái mới. Descriptor nhận được, tải xong, decode xong, DB commit và partition complete là các mốc khác nhau. isLast sớm không thay đủ batch. Không báo đủ history khi thiếu batch/phạm vi. Không chế phần trăm khi chưa biết mẫu số.

## 5. Banner và popup theo trạng thái thật

Lập bảng trạng thái → compact/full → vị trí → nội dung → progress → CTA → đóng. Bao gồm suggest, wait confirm, prepare, conv/preview, remaining messages, interrupted, error/retry và success. Bản compact có thể cố ý dùng lỗi chung; bản đầy đủ có mô tả theo nguyên nhân. Suggest full có nội dung từ server.

V2 nằm dưới filter và trên danh sách cuộn; legacy là phần tử đầu list. Không trộn kích thước hai loại. Popup đang hiện có thể suppress banner. Search/tab/priority quyết định hiển thị. Không làm tràn hoặc cắt chữ mép sidebar.

Mọi nút nối UI → WorkerCmd → Network/DB → GuiEvent payload → state → render. Đóng có thể hide/clear error/cancel theo trạng thái; không dùng một callback cho tất cả. Không chỉ đổi status_msg hoặc tự đặt thành công.

## 6. Kiểm thử và hồ sơ audit

Test offline trên dữ liệu tổng hợp với data dir riêng; không dùng live session/DB trong cargo test. Test đổi config phải assert consumer behavior, fallback, account reset và response đến muộn. Test startup, DB failure, cursor, batch trùng/đảo/thiếu, cancel/restart/reconnect và realtime xen history. Giữ các test có ích đang có.

Mỗi phần chạy check/test tập trung. Cuối tích hợp chạy cargo fmt, cargo check --locked, cargo test --locked, cargo clippy --all-targets -- -D warnings và git diff --check; kiểm tra exit code và ghi failed/ignored. Đặt USERPROFILE/.cargo/bin trong PATH và CMAKE_POLICY_VERSION_MINIMUM=3.5 khi cần. Trước commit/push phải chạy prepush gate hoặc cargo check theo AGENTS; không tự phát hành.

Quan sát native cùng kích thước/DPI và trạng thái. Các ca cần điện thoại phải phối hợp với người dùng; không giả là đã nghiệm thu. Không xóa dữ liệu thật để tạo queue evict/new login. Log dùng whitelist, không cookie/key/IMEI/raw URL/chat payload. Capture thô ở ngoài Git.

Bàn giao gồm: file thay đổi của bạn so với baseline; bảng requirement→code→test→native evidence; sơ đồ state startup/sync; bảng config consumer đầy đủ; DB nguyên nhân/bản sửa/phép đo; lệnh thực sự chạy; build/scenario native; các phần chưa xác minh. Ghi đúng đã có mã/qua test/xác nhận thiết bị.

Codex sẽ audit kỹ: config chỉ parse không apply, hardcode còn sót, session clone bị bỏ, thiếu owner/runtime, cursor trước commit, task cũ, timeout thành success, coverage giả và banner không nối backend.
