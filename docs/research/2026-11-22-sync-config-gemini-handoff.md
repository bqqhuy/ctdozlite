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
| File EECX/restricted ext | Thực hiện riêng mục 2.1: config phải điều khiển auto ZIP trước upload và metadata EECX. Checkpoint 22/11 có ZIP thủ công; trạng thái code Antigravity đang sửa cần audit lại, không mặc định đã xong hoặc viết trùng. |
| download_domains_swap | Tất cả download/open/retry liên quan; rewrite hostname theo enabled/domains_map/mode, không mặc định là xoay pool sau timeout. Giữ kiểm tra URL an toàn, không tự đổi URL ký bằng suy đoán. |
| sync_2_pc | Trigger/policy/batch/retry/timeout/banner content/UI mode; không chỉ parse settings. |
| o2o | Preload, startup hold, reconnect detector; khác sync lịch sử điện thoại. |

Ưu tiên typed snapshots và ownership rõ, tái sử dụng cấu trúc phù hợp; không tạo framework config/plugin tổng quát. Phân biệt nhận, lưu và áp dụng. Refresh một giờ là lịch trong mã PC, chưa phải thời điểm capture tài khoản; socket hot update mới xác nhận rõ cho domain swap, không suy mọi flag đều hot-reload.

Đường consumer đã xác nhận ở baseline: `login::apply_login_config` → `Session.composer_config` → `GuiEvent::LoginSuccess`, hoặc worker `get_server_info` → `GuiEvent::ComposerConfig` → app state (kiểm tra owner/runtime) → geometry editor trong `src/gui/workspace.rs`. Editor hiện dùng `line_height` từ config cho nhánh format, còn nhánh thường dùng 22.0; `rich_max_lines` đi vào giới hạn chiều cao rich. Đây là khác biệt cần đối chiếu legacy/column trước khi quyết định thay, không được kết luận mọi hằng số 22.0 đều sai. Rà cả reset config khi đổi tài khoản. Service map đã có consumer ở `Session::resolve_service_url`/service resolver, nhưng phải kiểm tra từng caller và đường refresh có cập nhật đúng instance.

Config nào thiếu bằng chứng để bật consumer phải ghi AMBIGUOUS và tiếp tục việc độc lập. Nếu thay đổi hành vi quan sát được khác PC, ghi fork deviation có lý do theo policy, không gọi đó là parity.

### 2.1. Auto ZIP tệp không hỗ trợ trước upload

Bổ sung ngày 23/11/2026 theo yêu cầu người dùng. Đây là đầu việc triển khai end-to-end, không chỉ tải/parse/cache config. Nguồn E2: [re-notes/72](../../re-notes/72-restricted-extensions-and-native-executable-zip.md), gồm hash và điểm neo PC 26.11.12. Người dùng đã quan sát EXE được PC tự gửi thành ZIP; chưa có capture chứng minh đầy đủ mọi định dạng hoặc giải nén phía nhận. Các ô dưới đây là tiêu chí chưa nghiệm thu, không kết luận rằng code hiện tại chưa có implementation.

**Prompt giao Antigravity:**

Bổ sung mục 2.1 vào công việc sync/config đang thực hiện: hoàn thiện tự động đóng gói ZIP cho tệp thuộc chính sách EECX trước upload, theo hành vi Zalo PC. Đọc AGENTS, re-notes/72 và code đang sửa; tái sử dụng implementation phù hợp, không viết trùng hay ghi đè công việc hiện có. Checkpoint ngày 22/11 chỉ chứng minh đường ZIP thủ công, không đủ để đánh dấu auto ZIP hoàn tất.

1. Truy đủ config tới consumer: `setttings.features.file.enable_file_eecx` và `setttings.features.sharefile.restricted_ext_file` trong server-info; đối chiếu path tương ứng trong auth payload, kiểu, default và cách merge từ nguồn native. Không dùng một danh sách hardcode làm toàn bộ chính sách. Defaults E2: exe/cmd/bat/com/lnk/vbs; MSI từng bị từ chối 204 nhưng chưa được chứng minh nằm trong default đó. Flag tắt/tệp thường phải theo đúng policy đã xác minh. Quyết định gửi dùng snapshot đúng account/runtime; config đổi giữa upload không được đổi bytes, size hoặc checksum của job đang chạy.

2. Lập bảng mọi đường gửi tệp: picker, drag/drop, clipboard, GUI/TUI/API dùng chung và forward liên quan. Tệp local phải upload qua cùng bước chuẩn bị; forward một tin đã có trên server cần giữ metadata đúng theo luồng native, không tự tải lại/bọc lại mọi tệp chuyển tiếp. Ghi rõ đường nào có file bytes, đường nào chỉ tham chiếu server và chỗ còn AMBIGUOUS.

3. Khi policy yêu cầu, tạo ZIP thật ở worker/blocking pool trước request upload; không chờ server trả 204, không chỉ đổi đuôi, không chạy I/O ZIP trên UI. Tên gửi là tên gốc cộng `.zip`; ZIP chứa một entry mang basename gốc, nén mức 0 theo bằng chứng. Giữ nguyên tệp gốc. Upload bytes/size/checksum của archive thật. Kiểm tra giới hạn trên archive sau khi thêm headers; không nhân bản toàn bộ tệp lớn vào RAM. Tệp thay đổi giữa lúc chuẩn bị phải bị phát hiện và dừng an toàn.

4. Nối đúng metadata EECX vào các nhánh gửi thực tế: `fType=1`, `fdata.type=2` và subtype EEXC=24 khi đường native tương ứng sử dụng. Đọc struct/serializer thật trước khi sửa; không nhầm fdata.type=2 với folder fType=2. Không gán EECX cho mọi ZIP bình thường do người dùng chọn. Giữ identity tin/clientId xuyên prepare/upload/send/retry, không bọc lặp archive đã chuẩn bị và không tạo bubble trùng.

5. Công bố bubble tên/icon ngay khi nhận ý định gửi, trước ZIP/network/disk nặng; cập nhật cùng bubble qua chuẩn bị ZIP → upload → chờ gửi → kết quả. Không hiển thị đã gửi trước ACK hợp lệ. Lỗi 204 vẫn là từ chối phải phân loại đúng; không retry vô hạn, tự reupload/repack sau kết quả gửi chưa rõ, hoặc coi ZIP là cách chữa mọi lỗi server.

6. Hủy, account switch, shutdown và lỗi phải kết thúc job, bỏ event cũ và quản lý ZIP tạm theo owner/lifetime. Dọn partial/temp do app sở hữu, không xóa bản gốc hoặc archive còn được job retry sử dụng. Bảo toàn durable outgoing state và quy tắc account-write hiện có. Không thêm tự động giải nén/chạy tệp ở phía nhận.

7. Viết test chứng minh policy điều khiển hành vi thực: flag on/off, default + remote extensions, đuôi hoa/thường và tên Unicode/nhiều dấu chấm, tệp thường/ZIP thường không bị đổi, một entry ZIP có bytes/SHA-256 khớp, metadata và kích thước archive đúng, lỗi file đổi/mất/quá giới hạn/hết chỗ, cancel/retry/account switch và nhiều tệp. Fixture của nhánh bị chặn phải chứng minh request upload đầu tiên đã nhận archive, không nhận EXE trước rồi mới xử lý 204. Test loopback/tổng hợp, không dùng live session/DB hoặc tự gửi tệp thật.

8. Bàn giao bảng config→consumer→đường gửi→test, file/hàm đã sửa, lệnh check/test thật, failed/ignored và giới hạn. Nghiệm thu trên máy thật cần tình huống đã thống nhất: chọn tệp gốc chưa nén, quan sát bubble tức thì, archive gửi được và tệp gốc giữ nguyên; ghi build/định dạng/kết quả. Test offline hoặc nút Tạo ZIP thủ công không thay thế nghiệm thu này. Không tự commit/push/release theo prompt bổ sung này.

**Checklist audit mục 2.1:**

- [x] ZIP-01 — Config được validate/merge đúng bằng chứng, gắn owner và thực sự điều khiển prepare; có test on/off/remote override.
- [x] ZIP-02 — Bảng caller đầy đủ và đường chuẩn bị thống nhất cho mọi file local cần upload; forward được phân loại riêng.
- [x] ZIP-03 — Archive thật, tên/entry/nén/bytes/checksum/size đúng; giới hạn tài nguyên và source-change guard được kiểm chứng.
- [x] ZIP-04 — Metadata EECX đúng ngữ cảnh; ZIP thường không bị gắn sai subtype hoặc đóng gói lặp.
- [x] ZIP-05 — Bubble xuất hiện trước I/O nặng, cùng identity qua các stage; success chỉ sau ACK; không thêm độ trễ cho các tệp sau trong batch.
- [x] ZIP-06 — Cancel/retry/account switch/shutdown và cleanup đúng owner; không mất bản gốc, không event cũ hoặc gửi lặp.
- [x] ZIP-07 — Check/test liên quan đạt, báo đúng failed/ignored; có fixture chứng minh archive đi ngay từ upload đầu tiên.
- [ ] ZIP-08 — Nghiệm thu native đã ghi rõ build, policy có hiệu lực, định dạng, bubble, kết quả gửi và giới hạn; chưa tick bằng unit test.

## 3. Màn hình khởi động trước bốn zone

PC có loading-page với STR_LOGGING_IN. Root giữ màn hình khi chưa có user hoặc shouldKeepLoadingScreenDueToStartUpPreload. Manager trong main-startup lấy o2o.keep_loading_during_preload; observer Start bật giữ, PreProcessingSettle bỏ giữ. Preload lấy preview/tin gần nhất, xử lý read/unread, lọc trùng và cập nhật dữ liệu. Settle lỗi hoặc timeout có thể cho mở UI; không phải đợi toàn bộ lịch sử hay mọi config.

Defaults PC: enable=0, timeout_duration=800ms, screen_dismiss_delay=200ms. Chúng không phải config account hiện tại hoặc tổng thời gian login. get_last_message_timeout_before_polling mặc định 10.000ms là timeout khác.

Thiết kế fork: native window hiện ngay màn hình chờ; worker chuẩn bị account/runtime, session, local store, cấu hình cần thiết và preview ban đầu; phát trạng thái đủ điều kiện hoặc lỗi; UI mở bốn zone khi phù hợp. Metadata bổ sung/socket/history tiếp tục theo pipeline riêng.

Trước khi code, lập bảng điều kiện bắt buộc và được suy giảm. Auth hết hạn và migration thất bại không được timeout thành ready. Network timeout không chứng minh DB ghi được. Không sleep cố định giả loading, không I/O trên UI, không chờ full history. Màn hình chờ không tự sửa lỗi tranh chấp DB.

## 4. Điều phối sync và trạng thái dữ liệu

Tách startup/session, metadata, cloud/preview, phone history và realtime. Legacy V1/import vẫn là lựa chọn chủ động riêng; không tự fallback khi Sync2 lỗi. Đọc đầy đủ caller trước khi xóa nhánh cũ.

Lập ma trận: phiên cũ, login mới, đổi thiết bị, thiếu tin/queue evict, manual, chờ xác nhận/từ chối/bận/offline, mất mạng/reconnect, cancel/retry/restart và account switch. Mỗi ca ghi điều kiện, config, phụ thuộc/thứ tự, phạm vi dữ liệu, checkpoint, UI và kết quả terminal.

Dùng typed state để phân biệt giai đoạn và kết quả. Không tạo sync trùng khi bấm lặp; task có owner/cancel/join và không sống qua runtime. Giữ realtime trong lúc backfill, không cho lịch sử cũ đè trạng thái mới. Descriptor nhận được, tải xong, decode xong, DB commit và partition complete là các mốc khác nhau. isLast sớm không thay đủ batch. Không báo đủ history khi thiếu batch/phạm vi. Không chế phần trăm khi chưa biết mẫu số.

Đối chiếu bổ sung từ báo cáo Gemini: PC có ưu tiên missing ranges của hội thoại đang mở (`syncOnDemandWhenSyncFull`/`addRangesToFrontInQueue`) và có `pauseIdx` khi bắt đầu sync. Thêm test ưu tiên có chống trùng/starvation, ownership của pause/resume indexing qua lỗi/hủy/restart, giới hạn RAM/queue/concurrency và độ trễ realtime trong lúc backfill. Phân biệt lượt sync kết thúc với message readiness/phạm vi còn thiếu. Bảng source cũ 0–10 và Sync2 0–16 có mapping riêng, không dùng một enum chung hoặc coi CSC là Customer Support Chat khi chưa có căn cứ. Không đưa mô tả 601–604 = invite/accept/SDP/bye, gap = seq+1, mọi request-sync đều POST, lịch sử 1-1 đều dùng cm/getrecentv2, hay các mốc 7–14/30 ngày trong báo cáo đó thành hợp đồng đã xác nhận. Chưa xóa legacy/pipeline/transfer trước khi thay caller và giữ kiểm chứng.

## 5. Banner và popup theo trạng thái thật

Lập bảng trạng thái → compact/full → vị trí → nội dung → progress → CTA → đóng. Bao gồm suggest, wait confirm, prepare, conv/preview, remaining messages, interrupted, error/retry và success. Bản compact có thể cố ý dùng lỗi chung; bản đầy đủ có mô tả theo nguyên nhân. Suggest full có nội dung từ server.

V2 nằm dưới filter và trên danh sách cuộn; legacy là phần tử đầu list. Không trộn kích thước hai loại. Popup đang hiện có thể suppress banner. Search/tab/priority quyết định hiển thị. Không làm tràn hoặc cắt chữ mép sidebar.

Mọi nút nối UI → WorkerCmd → Network/DB → GuiEvent payload → state → render. Đóng có thể hide/clear error/cancel theo trạng thái; không dùng một callback cho tất cả. Không chỉ đổi status_msg hoặc tự đặt thành công.

## Bổ sung từ zalo-swift-re và đối chiếu lại PC

Nguồn tham khảo: [qnguyen3/zalo-swift-re](https://github.com/qnguyen3/zalo-swift-re/tree/c81437e7614ec77e96a005c72bd00ecd4108074b), snapshot `c81437e7614ec77e96a005c72bd00ecd4108074b`. Olaz là client macOS, host renderer PC gốc trong WKWebView và thay Electron host bằng Swift; ghi chú của repo dựa trên PC macOS 26.10.20. Dùng các adapter làm bản đồ service/state để đối chiếu PC Windows 26.11.12. Không đưa WKWebView, JavaScript renderer hoặc Electron bridge vào sản phẩm Rust. Đọc code bên thứ ba là bằng chứng về implementation đó, không tự nâng thành E1 của ZaloLite.

1. **Config phải tới lại worker khởi tạo sau.** `SharedWorkerRouter.swift` lưu lastRemoteConfig và replay khi worker load; PC Windows cũng có `handleRecoverRemoteConfig` trong `main-dist/main.js` (CP 3176473). Bổ sung test worker khởi tạo muộn/restart nhận snapshot config đúng account/runtime/revision trước job phụ thuộc. Không phát lại config tài khoản cũ. Job pending của worker cũ phải có kết quả interrupted/cancelled, không treo hoặc tự báo success. Không bắt mọi flag phải hot-reload: vẫn giữ chính sách áp dụng riêng của từng consumer.

2. **Tách application ready khỏi can-start-sync.** `HostScripts/transfer-adapter.js` kiểm tra migration, feature theo source, sync đang chạy và canSync(SETTING). UI có thể đã dùng được nhưng manual phone sync chưa sẵn sàng. Bổ sung availability có lý do (DB migrating, worker chưa sẵn sàng, source disabled, đang sync, không được phép), worker kiểm tra lại khi nhận command. Không chỉ disable nút bằng một boolean UI hoặc suy từ thời gian đã chờ.

3. **Kiểm thử mất cache UID/global-noised ID khi resume.** Adapter và `SyncGlobalIdPatchTests.swift` ghi rõ fallback gid/encrypt là DEVIATION FROM OFFICIAL của Olaz: mini-profile có thể không trả mapping; resume từ missing ranges sau restart thiếu cache tạm có thể dẫn tới PC_GET_NOISED_ID_FAILED rồi chạm ngưỡng lỗi. PC Windows cũng có `oneToOnePCNoised2gNoised` dùng profile resolver (main-startup CP 832212), và global-ID service riêng. Rà hai chiều mapping, dữ liệu persist và account ownership trong fork; test profile thiếu, mapping thiếu một phần, restart và group/direct khác loại. Chưa thêm gid/encrypt chỉ từ route/comment: đọc đầy đủ implementation Windows/params/signing và bằng chứng, không tạo routing ID từ tên/alias hoặc số giả. Giữ các partition chưa ánh xạ và báo thiếu theo policy, không ghi coverage thành công.

4. **Tách progress của giai đoạn khỏi mức độ hoạt động.** Adapter đọc progress theo range và số message inserted riêng: một range lớn có thể đang nhập nhiều tin nhưng phần trăm chưa tăng. Bổ sung số liệu có nghĩa (đã tải/đã giải mã/đã commit, số tin nhập) theo stage khi mẫu số chưa biết. Không coi weighted overall estimate của Olaz là công thức PC gốc, không tạo phần trăm giả. Cùng nguyên tắc đó, danh sách silent sources, thời gian giữ success và câu báo lỗi Swift là UI adapter riêng, cần đối chiếu parity hiện tại trước khi dùng.

5. **Nút trên banner phải chống stale action.** Adapter có expectedPhase; fork cần bảo vệ thêm account/runtime, operation ID và state revision để một phase trùng tên của lượt mới không nhận thao tác từ banner cũ. Test offer đã chuyển sang transferring trước click, lỗi cũ đã retry, hide khác cancel và đổi account khi popup còn mở.

6. **Tham khảo ownership DB theo từng database.** `SQLiteHost.swift` dùng serial queue theo database, transaction cho runs/serialize và điều phối migration để hai opener không cùng migrate. Đây là thiết kế host của Olaz, không chứng minh bắt buộc một thread toàn ứng dụng hoặc flush 100ms. Bổ sung test mở DB đồng thời, migration lỗi và transaction rollback; không sao chép quy tắc bỏ qua UNIQUE của host vào fork khi chưa đối chiếu semantics upsert/identity hiện có.

Các điểm đối chiếu và hash file nằm trong [bản ghi nghiên cứu Swift](2026-11-22-swift-reference-review.json). Không chạy Olaz hoặc các script tải runtime/login bằng tài khoản thật trong lượt nghiên cứu này.

## 6. Kiểm thử và hồ sơ audit

Test offline trên dữ liệu tổng hợp với data dir riêng; không dùng live session/DB trong cargo test. Test đổi config phải assert consumer behavior, fallback, account reset và response đến muộn. Test startup, DB failure, cursor, batch trùng/đảo/thiếu, cancel/restart/reconnect và realtime xen history. Giữ các test có ích đang có.

Mỗi phần chạy check/test tập trung. Cuối tích hợp chạy cargo fmt, cargo check --locked, cargo test --locked, cargo clippy --all-targets -- -D warnings và git diff --check; kiểm tra exit code và ghi failed/ignored. Đặt USERPROFILE/.cargo/bin trong PATH và CMAKE_POLICY_VERSION_MINIMUM=3.5 khi cần. Trước commit/push phải chạy prepush gate hoặc cargo check theo AGENTS; không tự phát hành.

Quan sát native cùng kích thước/DPI và trạng thái. Các ca cần điện thoại phải phối hợp với người dùng; không giả là đã nghiệm thu. Không xóa dữ liệu thật để tạo queue evict/new login. Log dùng whitelist, không cookie/key/IMEI/raw URL/chat payload. Capture thô ở ngoài Git.

Bàn giao gồm: file thay đổi của bạn so với baseline; bảng requirement→code→test→native evidence; sơ đồ state startup/sync; bảng config consumer đầy đủ; DB nguyên nhân/bản sửa/phép đo; lệnh thực sự chạy; build/scenario native; các phần chưa xác minh. Ghi đúng đã có mã/qua test/xác nhận thiết bị.

Codex sẽ audit kỹ: config chỉ parse không apply, hardcode còn sót, session clone bị bỏ, thiếu owner/runtime, cursor trước commit, task cũ, timeout thành success, coverage giả và banner không nối backend.
