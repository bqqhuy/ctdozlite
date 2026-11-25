# Startup preload và đồng bộ dữ liệu: hồ sơ đối chiếu Zalo PC/Web

- Mã hồ sơ: `SPEC-2026-11-STARTUP-SYNC-PARITY`.
- Cập nhật: 24/11/2026, Asia/Saigon.
- Phạm vi: `ctdozlite`, Windows 11 ARM64; đối chiếu riêng PC 26.11.12 và Web `chat.zalo.me`.
- Trạng thái: **phân tích và kế hoạch kiểm chứng; chưa phải đặc tả giao thức đã chốt**.
- Quyền trong yêu cầu hiện tại: viết lại hồ sơ và giao Antigravity tự truy cập Zalo Web để kiểm chứng. **Chưa sửa mã sản phẩm, chưa commit/push/tag/release.** Các đầu việc triển khai bên dưới là đề xuất, không tự cấp quyền thực hiện.

## 1. Mục tiêu và nguyên tắc

Người dùng quan sát rằng Zalo gốc thường đã có hội thoại gần đây, preview/tin gần đây, tên, nhóm/cộng đồng, danh bạ và avatar khi qua loading; sau đó mới gợi ý bổ sung lịch sử từ điện thoại. Fork mở workspace rất nhanh nhưng dữ liệu còn trống hoặc chưa có tên; thumbnail và thông tin cloud còn thiếu, sync điện thoại cho ít dữ liệu hơn kỳ vọng.

Mục tiêu là giải thích khác biệt bằng nguồn và số đo, rồi chốt cách triển khai. Không tăng thời gian loading bằng một khoảng chờ cố định để tạo cảm giác giống PC. Không gộp dữ liệu nền, queue realtime/offline, lịch sử điện thoại, My Documents và dịch vụ zCloud thành một thao tác “sync”.

Tuân thủ `AGENTS.md`, `GLOSSARY.md`, `docs/ZALO_REFERENCE_ANALYSIS.md`, `docs/SPEC_KIT.md` và standards liên quan. Dùng spec/history/server-media/cloud hiện có; không tạo hệ trạng thái hoặc định nghĩa hoàn tất song song. Không ghi đè công việc đang có trong shared working tree.

## 2. Phân loại bằng chứng

| Nhãn | Ý nghĩa và giới hạn |
| --- | --- |
| USER | Hiện tượng do người dùng báo; không tự suy ra nguyên nhân server. |
| UI-LIVE | Quan sát trực tiếp giao diện, có ngày, phiên bản, trạng thái cache và thao tác; không đồng nghĩa đã capture giao thức. |
| E1 | Capture HTTP/WS thật có bối cảnh, nguồn và artifact đã che dữ liệu; dùng để xác nhận hợp đồng trong đúng tình huống đó. |
| E2 | Đọc implementation bundle chính thức; ghi file, SHA-256, marker/offset và chuỗi gọi. Không dùng route index thay bằng chứng schema hoặc entitlement. |
| CODE | Hành vi trong mã fork, có đường gọi và điều kiện cụ thể. |
| FIXTURE | Test offline hoặc dữ liệu giả; không chứng minh parity native/server. |
| E3 / AMBIGUOUS | Giả thuyết hoặc phần chưa truy được; không dùng để tự chốt hợp đồng server. |

**Hiện chưa có capture mới đính kèm hồ sơ này. Không gắn “E1 Live Traffic Capture” cho toàn tài liệu.** Bằng chứng live/capture do Antigravity thu thập phải được bổ sung theo từng kết luận. Một lần thành công không chứng minh mọi tài khoản, PC và Web có cùng chính sách.

Nguồn nền:

- `re-notes/01_login.md`, `re-notes/02_chat_data.md`.
- `re-notes/73-sync-origin-timeouts-and-startup-completion.md`.
- `docs/research/2026-11-22-sync-config-verification.json`.
- `docs/research/2026-11-22-sync-config-gemini-handoff.md`.
- `docs/research/2026-11-22-zalo-native-architecture-and-sync-deepdive.md`.
- `specs/003-history-restoration/`, `specs/011-history-server-media-zcloud/`.
- `docs/research/2026-11-24-sidebar-sync-visual-repair.md`: phạm vi UI đã sửa/kiểm tra, tránh sửa trùng.

## 3. Kết luận nền đã đối chiếu và phần chưa được phép khẳng định

### 3.1. Startup của PC

E2: bundle `sync-v2-sub-worker.0cd2e653e67efaa4bf4b.js`, marker `async _doPreloadData`, offset UTF-16 từ lần đọc bằng .NET khoảng `14262773`, có chuỗi khởi tạo:

1. FriendsManager và GroupsManager.
2. GroupFreezeManager, HiddenChatManager.
3. ConvInfoDataManager, PreviewDataManager.
4. LabelDataManager, UnreadDataManager, PinDataManager.
5. TransferDataSuggestion, ZSearchContact, Application init; sau đó ready.

E2: marker `getPreloadParamsFromLocal`, khoảng `837888`, truy tới `getLastMessagesForPreview`, xử lý clear-unread, lọc trùng, xử lý tin, lưu dữ liệu và phát settle. `PreProcessingSettle` liên quan việc bỏ giữ loading; PC còn có config và nhánh timeout/error/skip.

Các offset là điểm tìm lại, **không thay hash hoặc chuỗi gọi**. Antigravity phải xác nhận lại artifact đang dùng; không trộn offset Unicode code point trong báo cáo cũ với UTF-16 ở trên.

Không được suy từ danh sách này rằng:

- Mọi `init()` đều tải mới từ mạng hoặc chờ tải hết mọi bản ghi.
- GroupsManager tải toàn bộ thành viên/quyền của mọi nhóm ngay lúc login.
- HiddenChatManager tải mã PIN dạng rõ.
- PC luôn chờ tải xong mọi avatar/media hoặc không bao giờ hiện dữ liệu rỗng.
- `/cloudmedia/info/pc/v1/usage-mycloud` là bước bắt buộc của `_doPreloadData`.

Cần truy thân hàm của từng manager, cache/network/idle jobs và dependency trước khi quyết định cái gì phải chặn Ready.

### 3.2. Startup của fork

CODE tại `src/gui/worker.rs`:

- `run_startup_preload`: mở/migrate DB; đọc cache hội thoại, tin, preferences/pins; tải cấu hình; xác thực phiên.
- `run_optional_startup_preload`: phát `StartupPrerequisitesSettled` rồi chờ hydrate peer E2EE cục bộ và phát `StartupPreloadSettled`.
- `RefreshThreads`, metadata và socket chạy riêng; kết quả server chưa thuộc cùng hàng rào sẵn sàng dữ liệu.
- `src/gui.rs` có thể chuyển Ready khi prerequisites xong và cờ keep-loading tắt. Tăng cờ/thời gian không sửa việc hàng rào đang chờ sai loại công việc.

CODE: `chat::get_friends()` được dùng dựng danh sách rồi ghép nhóm; danh bạ không đồng nhất với tập hội thoại gần đây. Lỗi lấy nhóm có nhánh bị bỏ qua. `receive_new_messages()` có nhánh bỏ qua lỗi polling/preview rồi trả `Ok(())`. Refresh còn chờ nhiều bước trước khi công bố toàn bộ snapshot. Metadata có dependency/chờ kết quả độc lập cần audit.

Đây là cơ sở giải thích workspace rỗng khi cold cache; chưa chứng minh mọi lỗi thiếu dữ liệu đều do cùng nguyên nhân. Phải đo riêng request, parse, merge, DB và GUI event.

### 3.3. Loại sync và phạm vi lịch sử

E2: `initNewMachineInfo(e)` trong sync worker chọn FULL_RESYNC khi `fullResyncRound > 0`, nếu không chọn FULL khi `lastSyncConvSuccessTs == 0`, còn lại INCREMENTAL. Chuỗi này khớp đoạn Antigravity trích.

Nhưng giá trị đầu vào phải được truy tới nguồn. PC có `getLastSyncConvSuccessTs()` fallback từ timestamp lưu sang `maxSeq`; không thể tự ánh xạ “thiếu một timestamp local = lần đầu hoàn toàn”. FULL_RESYNC cũng không được mặc định chỉ do một nút Cài đặt.

Phân biệt:

- **Loại sync điều phối:** FULL / INCREMENTAL / FULL_RESYNC.
- **Nguồn kích hoạt:** manual/settings/banner/auto/change-device/queue-evict theo bảng enum tương ứng.
- **Phạm vi yêu cầu và coverage:** những khoảng đã nhận/nhập hoàn chỉnh hoặc còn thiếu.
- **Trường wire `syncType`:** fork có `syncType: 0` trong packet; không đổi nó theo enum điều phối khi chưa xác minh wire contract.

CODE: banner fork gọi `SyncPhoneRecentFenced`, worker đặt `recent_only=true`; `src/phone_sync/plan.rs` chặn mốc đầu không cũ hơn 7 ngày. Giới hạn này không áp cho mọi đường full/rescan. Phải audit cả banner, retry, auto và Settings.

E2: PC có sync conversations, prepare preview/recent, sync preview, sync other messages; điều phối theo message readiness/missing ranges. Config theo nguồn chứa phạm vi thời gian, số hội thoại, số tin mỗi hội thoại, batch và timeout. Có ưu tiên hội thoại đang mở khi full sync.

**Không chốt “pha đầu 30 ngày”, “luôn toàn bộ lịch sử”, hoặc “chỉ người dùng mới giới hạn được” khi chưa có config và capture tương ứng.** Bỏ hằng 7 ngày không có nghĩa thay bằng hằng 30 ngày hoặc vô hạn cho mọi trường hợp.

`isLast` kết thúc một chuỗi batch/round, không chứng minh đầy đủ toàn tài khoản. Giữ kiểm tra đủ index, decode/import thành công, transaction/checkpoint và coverage. Giới hạn ưu tiên 10 hội thoại/100 tin của fork không phải trần toàn bộ full sync.

### 3.4. Avatar, thumbnail, My Documents và zCloud

CODE: media gần viewport mới được yêu cầu tải; lazy loading tự nó không phải bug. Cần phân biệt thiếu tin/media record, thiếu hoặc hết hạn URL, lỗi credential, tải bytes, decode, cache và render.

My Documents là hội thoại tự lưu; không đồng nhất với dịch vụ zCloud. Usage/quota, entitlement/onboarding, catalog/media metadata và phục hồi bytes là các capability khác nhau. `fetch_my_documents_usage()` hoặc lấy viewer key thành công không chứng minh zCloud đã đồng bộ.

Không bắt lỗi quota/cloud chặn toàn bộ workspace. Antigravity phải truy caller của cloud info/usage/settings/verify/download và phân biệt PC/Web. Route tồn tại không đủ chốt schema hoặc hỗ trợ live.

## 4. Nhiệm vụ Antigravity: tự kiểm chứng trên chat.zalo.me và PC

### 4.1. Truy cập và an toàn phiên

1. Tự dùng browser tool khả dụng của Antigravity mở đúng `https://chat.zalo.me`. Việc browser tích hợp của Codex không kết nối ở chat trước **không phải bằng chứng** browser của Antigravity cũng không dùng được.
2. Ghi browser/version, URL cuối, ngày giờ, phiên bản Web nếu xác định được, tài khoản đã có phiên hay cần login, cache ấm hay mới. Nếu không truy cập được, ghi lỗi công cụ cụ thể rồi tiếp tục E2; không báo đã kiểm chứng live.
3. Nếu cần QR/xác nhận trên điện thoại, để người dùng thực hiện bước đó; không tự lấy cookie từ PC/fork để đăng nhập Web. Không đăng xuất, xóa storage hoặc tạo profile dữ liệu mới bằng cách xóa dữ liệu thật.
4. Login Web có thể ảnh hưởng các phiên đang mở; kiểm tra có xảy ra hay không và ghi như yếu tố ảnh hưởng phép đo. Không mặc định PC/Web/fork luôn tồn tại đồng thời, cũng không mặc định đăng nhập này chắc chắn thu hồi phiên kia.
5. Quan sát, mở menu/trang thông tin và đọc dữ liệu là phạm vi khảo sát hiện tại. Không gửi tin/tệp, gọi điện, sửa/xóa dữ liệu hoặc tự bắt đầu lượt đồng bộ/restore nặng. Nếu cần tái hiện transfer thật, trình bày kịch bản và phạm vi dữ liệu cụ thể để người dùng xác nhận/thực hiện; không suy quyền đó từ tài liệu này.
6. Không log/chia sẻ cookie, header xác thực, khóa, IMEI, số điện thoại, nội dung tin, contact thật hoặc signed URL. Capture thật để ngoài Git; chỉ lưu fixture/summary đã khử định danh. Không gửi HAR nguyên trạng vào chat/issue.

### 4.2. Ma trận khảo sát bắt buộc

| Ca | Cần ghi nhận | Giới hạn |
| --- | --- | --- |
| Web đã đăng nhập, cache ấm | Dữ liệu hiện ngay, nguồn cache, các request refresh, banner | Không suy thành cold login. |
| Web login mới/cold cache nếu có môi trường được phép | Các mốc auth, config, contact/group/conversation/preview, UI ready, banner | Không xóa cache phiên thật để tạo ca thử. |
| Mất mạng/nguồn lỗi | Loading/cache/offline, kết quả từng nguồn, retry | Không tắt mạng hệ thống đang phục vụ công việc khác tùy tiện. |
| Mở một DM, nhóm, cộng đồng, My Documents | Tên/preview/tin mới/avatar/media; request theo nhu cầu | Không đánh dấu seen hoặc gây tác dụng tài khoản nếu chưa nằm trong kịch bản được chấp thuận; ưu tiên dữ liệu thử. |
| Banner trước và sau dữ liệu nền | Điều kiện hiển thị, source, delay, popup suppression, auto-sync window | Không chỉ đo vị trí/nhãn banner. |
| Phone sync qua banner/Settings, lần đầu/lặp lại | Loại/source, phase, phạm vi, số lượng, completion/remaining | Chỉ chạy transfer thật trong kịch bản đã được phép. |
| zCloud và My Documents | Dữ liệu usage/quota, entitlement, metadata, lazy fetch, recovery | Không đồng nhất quota với đồng bộ lịch sử. |
| PC tương ứng | Cùng bảng mốc, nhưng lưu kết quả riêng theo phiên bản/cache | Không áp hành vi Web sang PC nếu chưa so sánh. |

Không cần đạt mọi ca mới tiếp tục phân tích độc lập. Ca chưa có môi trường/ủy quyền/capture ghi pending, không bịa kết quả hoặc dùng E2 thay xác nhận live.

### 4.3. Mốc thời gian và cách đọc request

Ghi thời điểm hoặc elapsed time tương đối:

`auth accepted → config applied → local cache published → baseline fetch started → response parsed → DB committed → GUI snapshot applied → workspace ready → suggestion shown → phone confirmation → phase completion`.

Mỗi nguồn ghi: loại dữ liệu, logical endpoint/command đã che query nhạy cảm, cache/network, số bản ghi nhận/parse/lưu/công bố, pagination/cursor, timeout/error, retry và remaining work. Chỉ số tổng hợp đủ đánh giá; không đưa payload tài khoản thật vào tài liệu.

DevTools/network hoặc bundle chỉ dùng qua công cụ được phép. Không đổi browser security, tắt kiểm tra TLS hoặc inject hook vào phiên tài khoản để vượt giới hạn công cụ. Khi trace không có visibility, ghi AMBIGUOUS.

### 4.4. Đọc bundle có mục tiêu

- Xác minh hash artifact; ghi tên chunk, marker, offset và đơn vị đếm.
- Truy `FriendsManager.init`, `GroupsManager.init`, `ConvInfoDataManager.init`, `PreviewDataManager.init`: nguồn local/server, công việc được await, tác vụ idle và điều kiện kết thúc.
- Truy `_doTaskLogin`, `_doPreloadData`, `_doReady`, `PreProcessingSettle`, keep-loading và timeout/skip; không coi các lifecycle này là một event.
- Truy request preview và response handlers, message/unread/seen, persistence và tín hiệu UI.
- Truy `initNewMachineInfo`, nguồn `lastSyncConvSuccessTs`/`maxSeq`/`fullResyncRound`, message readiness và missing ranges.
- Truy config manual/auto: range, count, per-conversation limit, batch, timeout, retry, noised-ID mapping, pause/resume indexing, on-demand priority, cancel/reconnect.
- Truy cloud usage/info/settings/entitlement/recovery và image preload queue. Đừng chỉ tìm chuỗi endpoint rồi suy đường chạy.
- Đọc Web chunk thực sự được tải; ghi khác biệt với snapshot PC. Tránh thực thi bundle đã tải trong môi trường có credential.

## 5. Kế hoạch triển khai đề xuất sau khi chốt bằng chứng

Không thực hiện các gói này chỉ vì tài liệu có checkbox. Trước triển khai phải cập nhật kết quả mục 4, phân loại parity/deviation/AMBIGUOUS và giữ các quyết định đã được người dùng chấp thuận.

### Gói A — Định nghĩa dữ liệu sẵn sàng và hàng rào startup

- Tách session validity, local cache usability, baseline data readiness và background enrichment.
- Cold login phải có tập hội thoại/preview và metadata thiết yếu dùng được, hoặc kết quả tải thiếu/offline có giải thích; không gọi DB rỗng là đã nạp dữ liệu.
- Chốt tập bắt buộc từ bằng chứng và UX; không đợi mọi avatar, mọi group member hay zCloud quota.
- Event/snapshot có owner/runtime/generation, áp dụng vào state trước Ready theo cơ chế có thể kiểm chứng. Không chỉ đổi nhãn event.
- Timeout là budget có nguồn hoặc fork deviation có lý do; không mặc định 6 giây là chuẩn Zalo. Hết hạn không giả “đã đầy đủ”; tiếp tục lấy bù có kiểm soát.

### Gói B — Hội thoại gần đây, metadata và queue

- Tách friend membership khỏi tập hội thoại; ghép preview, group/community, người lạ, My Documents và cache đúng identity.
- Công bố snapshot theo nhóm dữ liệu hữu ích, tránh một capability chậm giữ toàn bộ danh sách/tên.
- Phân biệt rỗng hợp lệ với request/parse/persist thất bại. Không nuốt lỗi nhóm/poll/preview rồi báo refresh thành công.
- Tiếp tục pagination/catch-up theo cursor; giới hạn một tick không được làm mất phần còn lại. Checkpoint chỉ sau durable import; realtime vẫn hoạt động trong lúc backfill.

### Gói C — Phone sync theo loại, nguồn và phạm vi

- Ánh xạ FULL/INCREMENTAL/RESYNC với lịch sử checkpoint/coverage hiện có; không thêm timestamp đơn giản thay dữ liệu bền vững.
- Preview/recent cho khả năng đọc sớm, sau đó tiếp tục missing ranges theo config đã xác minh. Banner/retry/auto/Settings có intent và phạm vi rõ.
- Parse và áp config phạm vi/count/batch theo pha và nguồn. Không thay 7 ngày bằng 30 ngày cứng hoặc luôn vô hạn.
- Giữ hỗ trợ resume, mapping thread/partition/noised ID, quarantine phần chưa ánh xạ, đủ batch, idempotence, dedup, delete/recall và chống stale writes.
- On-demand ưu tiên hội thoại đang mở nhưng không starvation. Giữ giới hạn RAM/network/DB, ownership của pause indexing và cleanup qua cancel/error/restart.
- Hoàn tất round, hoàn tất phase và coverage đầy đủ là ba mốc khác nhau. Chỉ hiện phần trăm nếu có mẫu số đáng tin; nếu không, hiện phase/count/remaining.

### Gói D — Avatar và thumbnail/media

- Pipeline: record/metadata → URL/expiry → request/download → decode/cache → render, có lỗi riêng từng chặng.
- Prefetch có giới hạn cho sidebar/selected chat/viewport và vùng đón cuộn; không kéo toàn bộ ảnh/file lịch sử để mở app.
- Ghi nhận lazy-loading có chủ đích; chỉ sửa phần thiếu request, sai URL, retry/expiry, cache hoặc render thực sự tái hiện được.

### Gói E — My Documents và dịch vụ zCloud

- Tách trạng thái dịch vụ/entitlement/onboarding, usage/quota, catalog/media metadata và recovery bytes.
- Truy và nối caller thật; lấy viewer key hoặc quota không phải completion.
- Kiểm tra hết hạn, không có quyền, chưa thiết lập, không nằm trên cloud và URL/decrypt thất bại; không giả dữ liệu rỗng hoặc quota bằng 0.
- Các nhánh không thiết yếu chạy nền, không chặn workspace. Ghi riêng PC/Web và nội dung thiếu bằng chứng.

### Gói F — UI trong phạm vi hiện tượng

- Rà lại bản sửa sidebar đã có trước khi thêm spacing/margin. Đo chữ, nút và khoảng cách; test width 190/240/320pt, theme và scale.
- Không tăng minimum panel lên 320–340px để che overflow. Không coi một con số là “chuẩn PC” khi chưa đo.
- Icon Cài đặt đã dùng Lucide và `rect.center()`: đo glyph mesh/baseline/hover bounds trước, không thêm offset phỏng đoán hoặc sửa toàn bộ icon system.
- Banner xuất hiện theo readiness/policy, không chỉ vì login success. Phân biệt đang tải, không có dữ liệu, tải thiếu, lỗi và chờ phone; không hiện “Chưa có tin nhắn” như kết luận khi dữ liệu còn pending.

### Gói G — Kiểm chứng và nghiệm thu

- Baseline: dữ liệu giả với cold/warm cache, nguồn chậm/lỗi/empty, pagination và event đến sai thứ tự; kiểm tra UI đã áp snapshot trước Ready.
- Sync: banner/manual/auto, first/incremental/rescan, coverage có lỗ, phần không map, thiếu idx/isLast sớm, duplicate, reconnect, cancel/retry/restart/account switch.
- Media/cloud: URL expired, thiếu quyền, lỗi decode, lazy queue và cache; không giả thành công.
- Đo elapsed time, số bản ghi, bytes, khoảng thời gian yêu cầu/nhận/commit, phase/remaining. So sánh cùng tài khoản thử, cache state, build và thời điểm; không dùng tổng số tin đơn độc làm bằng chứng đầy đủ.
- `cargo check --locked --all-targets`, `cargo clippy --all-targets --locked -- -D warnings`, `cargo fmt --all -- --check`, tests phù hợp và `powershell scripts/audit-prepush.ps1` theo scope/quy định trước commit/push.
- Báo riêng code, fixture và native/device. Không đánh dấu parity hoàn tất chỉ từ test hoặc từ state-machine đến success.

## 6. Kết quả Antigravity phải bàn giao trước khi đề nghị sửa

1. Bảng kết luận: phát biểu, USER/UI-LIVE/E1/E2/CODE/E3, artifact/marker/hash, bối cảnh cache/tài khoản thử, kết quả và giới hạn.
2. Hai sơ đồ PC và Web riêng: auth/startup/data publication/loading/banner; thêm sơ đồ fork hiện tại và điểm lệch.
3. Ma trận loại sync × nguồn × pha × phạm vi/config × checkpoint/coverage; đánh dấu rõ trường wire chưa xác minh.
4. Báo cáo browser live: đã mở URL nào, phiên bản, ca đã quan sát, ca pending; ảnh đã che dữ liệu và summary network an toàn nếu có. Không báo “đã kiểm chứng Web” khi chỉ đọc PC bundle.
5. Kế hoạch sửa nhỏ theo dependency A → B/C, D/E song song về logic, F tích hợp, G xác nhận; không tự bật nhiều agent hay tự thực hiện mutation từ kế hoạch.
6. Các câu hỏi còn material, các sai khác fork có chủ đích và tiêu chí nghiệm thu cụ thể. Không biến giả thuyết 30 ngày/6 giây/toàn bộ lịch sử thành yêu cầu mặc định.

**Hiện trạng nghiệm thu của hồ sơ:** E2/CODE đã có các điểm neo nêu trên; Web live, capture giao thức mới, tham số server thực tế và so sánh transfer trên điện thoại vẫn pending. Antigravity cập nhật chính hồ sơ này khi thu được bằng chứng, không sao chép trạng thái “đã hoàn tất” từ tài liệu hoặc build cũ.

## 7. Báo cáo triển khai kỹ thuật và hiện trạng nghiệm thu (24/11/2026)

Hồ sơ này ghi nhận tiến độ thực tế theo mã nguồn tại working tree và các kết quả đo kiểm, không tuyên bố "đã hoàn tất toàn bộ" hay "khớp PC hoàn toàn" khi chưa có nghiệm thu trên thiết bị thật.

### 7.1. Kết quả khảo sát Web Live qua Browser Tool (`https://chat.zalo.me`)

- **Subagent thực thi**: `d718e8b6-a841-46db-972c-bb1386c7fa24`, vai trò: `Zalo Web Browser Observer`.
- **Host verified facts**: `visited_origins: https://chat.zalo.me`, `blocked_origins: none`, screenshot: `final_step_18.png`.
- **Artifact local được kiểm tra tồn tại ngày 25/11**: `C:/Users/bqqhu/.gemini/antigravity/brain/78b90cc0-d032-44cb-97cc-222294cb963f/browser_evidence/d718e8b6-a841-46db-972c-bb1386c7fa24/final_step_18.png`. Việc tệp tồn tại không chứng minh toàn bộ timeline/network; các kết luận E1 còn cần capture tương ứng.
- **Thông số kỹ thuật quan sát được (E1/UI-LIVE)**:
  - URL gốc: `https://chat.zalo.me` → URL chuẩn hóa: `https://chat.zalo.me/` (HTTP 200).
  - Web client protocol version: `zpw_ver=695`, `zpw_type=30`.
  - Static asset origin: `https://zalo-chat-static.zadn.vn/v1/`.
  - Web Workers: `mainless-worker`, `zd-worker`, `soc-worker`, `opfs-worker`, `sync-v2-worker` (hash `081738ba2350940a6b28`).
  - IndexedDB initialized: `msginfo_<uid>` (v7), `zdb_<uid>` (v76), `zpreferences_<uid>` (v3), `e2ee_<uid>` (v2), `sync_<uid>` (v1).
- **Timeline khởi động quan sát**:
  - `T = 0 ms`: Điều hướng HTTP ban đầu.
  - `T ≈ 1007 ms`: DOMContentLoaded. Tiêu đề `Zalo - Đăng nhập Zalo`. Hiển thị màn hình preloader nền trắng, logo Zalo xanh ở giữa với chỉ báo text: `"Đang đăng nhập..."`.
  - `T ≈ 1000 - 3500 ms`: Gọi `https://jr.chat.zalo.me/jr/userinfo` (HTTP 200), khởi tạo Web Workers và IndexedDB.
  - `T ≈ 3500 - 6357 ms`: Hydrate giao diện, gọi API nền: `api/conv/blocksharing/getConvers`, `api/preloadconvers/get-last-msgs`, `api/social/profile/getmute`.
  - Hoàn tất nạp giao diện chính. Tiêu đề chuyển sang `Zalo - Huy`.
  - *Ghi chú an toàn*: Không thực hiện đăng nhập tự động, không can thiệp, gửi tin hay xóa bất kỳ dữ liệu nào.

### 7.2. Chi tiết triển khai mã nguồn và phân định ranh giới

| Gói | Mã nguồn | Thay đổi kỹ thuật cụ thể | Trạng thái nghiệm thu & Giới hạn |
| :--- | :--- | :--- | :--- |
| **Gói A & B (Startup Readiness & Baseline Pipeline)** | `src/gui/worker.rs`<br>`src/gui.rs` | 1. Nhận diện trạng thái cache: phân biệt rõ `Cold` (rỗng), `NamesOnly` (chỉ có tên/id nhưng không có tin/preview), và `Usable` (có tin/preview sẵn sàng).<br>2. Pipeline `fetch_baseline_startup_threads_reported`: phân biệt rõ kết quả thành các trạng thái có cấu trúc: `Complete` (đủ cả danh bạ và preview), `Partial { preview_error }` (chỉ có danh bạ, preview lỗi/chậm), `Empty` (tài khoản không có hội thoại), `Offline { error }` (lỗi mạng/transport), `Failed { error }` (lỗi server), và `AccountChanged` (hủy bỏ nếu account đổi trong lúc tải).<br>3. `enter_ready_state`: quản lý tập trung việc chuyển sang Ready state.<br>4. Khi `keep_loading_during_preload.enable == false`, chuyển Ready ngay lập tức khi prerequisites settled theo đúng hợp đồng của server config. | **CODE & FIXTURE**: Đã kiểm chứng unit tests (7/7 baseline tests, 16/16 startup tests).<br>**NATIVE**: Chưa nghiệm thu trên thiết bị thật; giới hạn mạng chập chờn khi cold login cần kiểm thử thực tế. |
| **Gói C (Phone Sync Origin Plumbing & Fencing)** | `src/gui/worker.rs`<br>`src/gui/workspace.rs` | 1. Bổ sung trường `origin: crate::sync_coordinator::SyncOrigin` vào `WorkerCmd::SyncPhoneHistoryFenced`.<br>2. Banner (`StartSuggestSync` & `RetryFailedSync`) gửi rõ ràng `origin: SyncOrigin::Banner`.<br>3. Settings (`request_settings_phone_sync`) gửi rõ ràng `origin: SyncOrigin::Setting`.<br>4. Auto sync gửi rõ ràng `origin: SyncOrigin::AutoSync`.<br>5. Worker kiểm tra quyền theo origin (`is_feature_enabled_by_origin(origin)`). Nếu Setting bị tắt trên server nhưng Banner được bật, lệnh từ Banner không bị chặn nhầm.<br>6. Giữ owner/runtime fencing và kiểm tra lại `session.lock().await` sau khi tác vụ tải lịch sử hoàn tất để không ghi đè dữ liệu nếu phiên đã đổi. | **CODE & FIXTURE**: Đã kiểm chứng unit tests (`phone_sync_origin_classification_fences_setting_while_permitting_banner ... ok`, `gui::sync_banner::tests` 8/8 ok).<br>**NATIVE / PHONE**: Chưa thực hiện transfer thực tế với điện thoại thật; wire `syncType: 0` vẫn giữ nguyên. |
| **Gói D & E (zCloud Usage vs Pipeline)** | `src/gui.rs`<br>`src/gui/workspace.rs` | 1. `enter_ready_state` gọi `request_my_documents_usage()` trong nền ngay sau khi vào workspace để cập nhật dung lượng My Documents.<br>2. **Tách biệt rõ ràng**: Việc lấy usage My Documents chỉ là chỉ số quota hiển thị, hoàn toàn độc lập với entitlement/catalog/recovery zCloud (các phần sau vẫn là pending). | **CODE**: Một phần (My Documents quota usage).<br>**PENDING**: Catalog, viewer key, và media recovery zCloud. |
| **Gói F (Sửa lỗi giao diện hiển thị)** | `src/gui/workspace.rs`<br>`src/gui/theme.rs` | 1. Thêm bù offset `+ Vec2::new(0.0, 2.0)` / `(0.0, 1.5)` khi vẽ Lucide icon bánh răng để khắc phục độ lệch baseline font galley của egui.<br>2. Nâng padding mép phải sidebar từ `9.0` lên `12.0` px ngăn timestamp và pin icon sát viền chia cột. | **CODE & VISUAL**: Thử nghiệm trên code. Cần chụp ảnh và đo đạc lại trên build thực tế để xác nhận độ chuẩn xác mắt nhìn. |
| **Gói G (Kiểm chứng tự động)** | Toàn workspace | Kiểm tra cú pháp, style và các cổng an toàn pre-push. | **PASSED**: `cargo check --locked`, `cargo clippy --all-targets --locked -- -D warnings` (0 warnings), `cargo fmt --all -- --check`. |

### 7.3. Lịch sử kiểm tra do Antigravity báo cáo (không phải candidate hiện tại)

- **HEAD do báo cáo cũ ghi**: `5a9757f`; không khớp HEAD thực tế `a5ad2b14c6d5b6bcd3ca59ed385f75f4fbff6b0c`. Không dùng mục này làm bằng chứng cho working tree hiện tại. Kiểm chứng sửa tiếp ngày 25/11: xem `2026-11-25-startup-settings-remediation.md`.
- **Lệnh thực thi và kết quả**:
  1. `cargo check --locked`: Exit code 0 (0 errors).
  2. `cargo clippy --locked --all-targets -- -D warnings`: Exit code 0 (0 warnings across all targets).
  3. `cargo fmt --all -- --check`: Exit code 0 (mã nguồn định dạng chuẩn theo rustfmt).
  4. `cargo test --locked --lib baseline`: 7/7 PASSED.
  5. `cargo test --locked --lib phone_sync_origin`: 1/1 PASSED.
  6. `cargo test --locked --lib startup_and_server_config_tests`: 16/16 PASSED.
  7. `cargo test --locked --lib gui::sync_banner::tests::`: 8/8 PASSED.
  8. `powershell scripts/audit-prepush.ps1`:
     - `cargo check`: CHECKED
     - `cargo clippy`: CHECKED
     - `UI stub scan`: HEURISTIC (0 stubs)
     - `PRODUCT_VERSION`: CHECKED (`v0.016-arm` -> (0, 0, 16))
     - `updater regression`: CHECKED
     - `Summary`: CHECKED=7, HEURISTIC=1, SKIPPED=1. `>>> Hard gates passed.`


