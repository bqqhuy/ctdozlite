# Kế hoạch cải thiện tương đồng Zalo PC/Web — bản hiệu chỉnh sau audit

**Mã tài liệu:** `CTDOZLITE-PARITY-MASTER-PLAN-EXEC-V2`  
**Ngày:** 24/11/2026 (Asia/Saigon). **Nền mã:** `0d62e22afe95a262aeeb136f5a9a2a9b65747726`.  
**Trạng thái:** đã hiệu chỉnh kế hoạch; chưa triển khai hoặc nghiệm thu các task mới bên dưới.

Bản này thay nội dung thực thi V1, giữ mục tiêu UX nhưng sửa các giả định hiện trạng, hợp đồng server và thứ tự xử lý theo kết luận ba lượt đánh giá độc lập, phản biện chéo trong chat. Không dùng tiêu đề “100% parity” hoặc số test để khẳng định tương thích toàn bộ. Các nghiên cứu trong thư mục riêng của Antigravity là đầu vào tham khảo; bằng chứng để triển khai phải có điểm neo tái kiểm tra trong repo hoặc artifact có phiên bản/hash.

Phạm vi sửa của lượt cập nhật này là tài liệu. Không có mã Rust mới, kiểm thử native mới, commit, push hay release từ việc cập nhật kế hoạch. V1 trước sửa được giữ tạm tại `%TEMP%/zalolite-parity-master-plan-before-v2-20261124.md` để đối chiếu; file tạm không phải nguồn chuẩn hay bản backup bền vững.

## 1. Nguồn chuẩn và cách đọc trạng thái

- [AGENTS.md](../../AGENTS.md), [quy chế UI/UX](../standards/ui-ux-consistency.md), [Reference Authority](../ZALO_REFERENCE_ANALYSIS.md), [Spec Kit](../SPEC_KIT.md).
- [Kế hoạch GUI](../ZALO_GUI_UX_OVERHAUL_PLAN.md), [handover GUI](../GUI_UX_HANDOVER_AI_GUIDE.md); lịch sử phase đã merge không thay nghiệm thu bản hiện tại.
- [Audit dossier sync/config](2026-11-23-sync-config-audit-dossier.md), [kế hoạch remediation](2026-11-23-audit-round3-remediation-plan.md), [phân tích PC/Web/Android trực tiếp](2026-11-22-direct-pc-web-android-analysis.md).
- Spec hiện hữu: [003 history](../../specs/003-history-restoration/spec.md), [004 desktop](../../specs/004-desktop-interaction/spec.md), [005 UI](../../specs/005-ui-customization/spec.md), [006 stranger](../../specs/006-stranger-inbox/spec.md), [007 call](../../specs/007-call-multidevice/spec.md), [008 account/group](../../specs/008-account-group-state/spec.md), [009 rich media](../../specs/009-rich-media-messaging/spec.md), [011 server media/zCloud](../../specs/011-history-server-media-zcloud/spec.md).
- ADR-0029 (file drop), ADR-0030 (search/server state), ADR-0031 (group/restoration), ADR-0034 (hidden PIN), ADR-0035–0038 (E2EE) vẫn có hiệu lực. Không viết lại quyết định đã accepted bằng một kế hoạch UI.

Phân biệt **chưa làm / một phần / đã có mã / qua kiểm thử / xác nhận trên thiết bị**. “Có mã” dưới đây là quan sát source, không phải xác nhận server hay native. Mỗi mục parity phải ghi E1/E2/E3, nguồn, phiên bản/hash, điểm neo, điều kiện áp dụng và điều chưa biết. Route tồn tại không chứng minh method/schema/entitlement. E3 không đủ chốt hợp đồng server.

Không đổi ID yêu cầu/task hiện có trong specs. Các mã `MP-*` trong tài liệu chỉ là nhóm công việc của master plan, phải liên kết sang task hiện hữu trước triển khai; kết quả thực ghi ở `verification.md` tương ứng, không tạo trạng thái riêng cho từng AI.

## 2. Hiện trạng và xử lý đủ 17 mục của V1

| Mục V1 | Hiện trạng / giới hạn bằng chứng | Hướng xử lý V2 |
| :--- | :--- | :--- |
| 1. Popup/menu | `workspace.rs` đã dùng `row_resp.context_menu`; lỗi popup dùng backdrop được tái hiện ở chat riêng trên test-0d62e22. | Sửa shared layer/input trước. Chọn modal/popover theo mục đích và bằng chứng; không xóa tất cả modal hoặc viết lại context menu đã đúng. |
| 2. Emoji | Mục tiêu hiển thị màu và giữ Unicode còn hợp lệ; chưa chốt renderer mới. | Kiểm tra đường render thực, tài nguyên/license/cache; giữ grapheme, ZWJ, variation selector, skin tone, cờ và ngữ nghĩa editor. |
| 3. Avatar nhóm | Cần đối chiếu custom avatar/fallback/composite theo loại nhóm và dữ liệu thật. | Tái dùng cache; không gọi mạng lúc paint; thiếu ảnh dùng fallback trung thực. Không xóa fallback trước khi có thay thế. |
| 4. Voice | Playback, waveform và phiên âm là ba capability khác nhau. | Waveform từ dữ liệu thật; không bịa biên độ. Quyền/API phiên âm phải xác minh riêng; route/cmd không đủ. |
| 5. File card | Có đường tải/mở được worker kiểm tra account/active/hidden/path/cache. | Đổi trình bày và tiến trình trên đường hiện hữu. Không thay bằng shared TEMP + mở trực tiếp bỏ policy. |
| 6. Poll/rich cards | Có module/luồng nhóm; chưa chứng minh mọi thao tác nâng cao dùng được. | Inventory read/detail/vote/create/update/delete/permission, nối payload thực và trạng thái lỗi; thiếu contract để AMBIGUOUS. |
| 7. System events | Enum khác namespace: internal `20` có Recall; archive `20` có tên ChatInfo nhưng chưa đủ decoder. | Ánh xạ theo nguồn + action + schema; không đổi mọi `msgType=20` thành system notice. |
| 8. Search | Có search worker/FTS và drawer tìm trong thread; không phải toàn bộ chỉ có popup rời. | Xác định gap của global inline search/4 tab. Server fallback là task riêng cần contract, debounce, cancellation và quyền. |
| 9. Composer/caption | `file_drop.rs` đã mang caption và gửi `SendDroppedFiles`; các guard lân cận còn bảo vệ metadata/path/modal/batch. | Giữ caption và ADR-0029; không xóa nguyên đoạn guard theo số dòng V1. Rà 15 capability ở mục 6. |
| 10. Drawer | Đã có subview/action; cần đo lại khác biệt 1-1/group/community. | Tái dùng action hiện hữu, quyền và result payload; số nút/340px là ứng viên cần đối chiếu. |
| 11. VoIP | Đã có Receive/Miss/Reject, preflight và watchdog; actor hiện xét sáu trường cùng UIN/UID32. | Tái hiện gap, không thu hẹp còn bốn trường/UIN-only hoặc gán mọi terminal thành answered elsewhere. |
| 12. Responsive | `workspace.rs` có compact <1120 và narrow <620. | Đây là điều chỉnh hành vi có sẵn; chưa chốt thay bằng mốc 1024/720. Tính budget nội dung trước. |
| 13. Metadata/retry | archive, unread marker, hidden, blocksharing, presence/mute đã có mã; recent-search cần kiểm kê riêng. | Không viết lại “sáu microservice còn thiếu”. Rà từng adapter, consumer, fixture và native; sửa nghĩa retry ở mục 5. |
| 14. Cắt chữ rải rác | Báo cáo mới ngày 24/11 bổ sung preview tệp chờ gửi ngoài banner sync: tên/khung sát biên có biểu hiện cắt chữ. Source banner có reservation CTA cố định 76; chưa chứng minh hai chỗ cùng nguyên nhân. | Rà bounds/clip/text budget tại từng vị trí và primitive dùng chung; sửa có bằng chứng, không áp lề cố định toàn giao diện. Preview tệp phải nằm trong composer. |
| 15. Filter tabs | Khoảng cách/tab width cần đối chiếu thực trên build, font và DPI. | Đo text + token padding; không coi số 14–16px là hợp đồng native. Giữ các menu/action hiện có. |
| 16. Icon nút tròn | Cần kiểm tra glyph/rect/hit target thực. | Dùng hệ Lucide/vector chung cho action icon; kiểm tra căn tâm bằng render, không tuyên bố “100%” do gọi `rect.center()`. |
| 17. Setting Sync | Đã có config typed và privacy adapter; V1 trộn feature gate, account pref và local pref. | Thực hiện inventory theo mục 5; không phát setter tổng quát hoặc dựng toggle ghi vào remote gate. |

Các điểm neo source là tên module/hàm; số dòng V1 không còn là chỉ dẫn xóa mã. Task bắt đầu phải mở định nghĩa struct và rà mọi producer/consumer đang dùng.

## 3. MP-00 — Baseline và lỗi popup dùng chung (P0, làm trước UI mở rộng)

### 3.1. Bằng chứng và phạm vi

Chat [Sửa lỗi treo giao diện popup](thread://01a20c29-05b8-7261-8e90-8f7bbfed6c74?hostId=local) đã ghi nhận trên test-0d62e22: Settings không nhận tab/X nhưng Escape đóng được; Labels mở lại → click ngoài → X làm dialog tối và mất click. Đây là **quan sát native lịch sử**, không phải phiên thử mới trong lần sửa kế hoạch.

Source `src/gui/modals.rs::render_modal_backdrop` tạo fullscreen Area nhận click ở `Order::Foreground`; Settings/Labels Window cùng order. Với egui **0.29.1** đang pin, `containers/area.rs` có click/pointer/newly-visible → `move_to_top`; `memory/mod.rs` giữ order qua frame. Vẽ dialog sau backdrop không bảo đảm order ở các frame tiếp theo. Đây là bằng chứng source cho cơ chế lỗi, không phải chứng minh mọi popup lỗi cùng một cách.

### 3.2. Công việc

1. `MP-00A`: ghi HEAD + diff/hash nguồn được review; lập capability inventory: implemented, UI wired, fixture checked, native checked, gap, source/uncertainty cho 17 mục. Không gán tên Reviewer A/B nếu không có reviewer thật.
2. `MP-00B`: inventory toàn bộ surface dùng chung: Settings, Labels, profile/contact, group, hidden PIN, preflight call, forward, safety, auto-delete/protection, picker/search/tools. Với mỗi surface ghi parent/child, anchor, loại modal/popover/window, layer, outside policy, Escape/X, focus return và lifecycle.
3. Rà `is_any_modal_open`, `is_workspace_input_gated`, thời điểm snapshot gate trong workspace; không coi mọi popover/tool là modal toàn workspace. Kiểm tra preflight có được gate đầy đủ, không chỉ sửa danh sách boolean để che lỗi.
4. Thiết kế invariant **workspace < backdrop < dialog < child popup** với ownership đúng; background bị chặn theo mục đích, foreground nhận input, close/cancel/runtime change không để ghost blocker.
5. Rà pointer press/release, wheel, keyboard/global shortcuts, focus, IME, drag/drop và clipboard. Input không được tạo Send/WorkerCmd hoặc đổi thread ở nền khi modal đang sở hữu nó.
6. Chọn giải pháp nhỏ phù hợp API pin sau khi đọc source. `set_sublayer` của egui 0.29.1 chỉ hỗ trợ một mức, quan hệ được lấy/xóa mỗi frame; không thiết kế chuỗi nested tùy ý. Không hứa API mới trước khi kiểm tra.

Không dùng backdrop không nhận input gây click-through, đẩy mọi dialog lên Tooltip, `move_to_top` một lần, thêm sleep/repaint loop/thread hoặc nâng egui để né nguyên nhân. Sửa shared primitive phải kiểm tra toàn bộ consumer; chuyển quick filter sang popover không thay thế sửa modal còn giữ lại.

### 3.3. Nghiệm thu MP-00

- Event-based test nhiều frame: mở → idle ít nhất ba frame → outside press/release → click trong/tab/X → mở lại. Kiểm tra hit target/side effect thực; test open→X đơn lẻ không đủ.
- Parent Settings → PIN/ComboBox/child popup → đóng child → parent còn thao tác và focus đúng; Escape/X/outside theo policy từng surface.
- Không gửi lệnh hoặc đổi thread nền; không rò blocker sau close, logout/relogin cùng UID, account switch, resize.
- Native Windows: lặp đúng hai chuỗi lịch sử, kiểm tra scroll, màn hình sát mép, DPI và light/dark. Ghi build/ảnh đã che dữ liệu/thao tác/kết quả; không lấy fixture làm native PASS.
- Chỉ task phụ thuộc shared overlay chờ MP-00; các nghiên cứu độc lập vẫn tiếp tục. Không mở rộng thành xóa mọi modal.

## 4. MP-01 — Banner sync, sidebar và ngân sách bố cục

### 4.1. Banner sync

Bảo toàn state machine/origin/gate/timeout hiện có. Lập bảng mọi `SyncBannerKind` đang hỗ trợ: trigger, nội dung từ config/fallback, icon/progress, CTA/stop/close, dismiss/reopen, terminal/error/retry, timestamp, owner/runtime. “Lần sync gần nhất” phải lấy từ dữ liệu có provenance; không dùng giờ render hoặc ví dụ cố định. Stop phải hủy đúng operation; idle/error không bấm phát lặp mutation khi kết quả trước chưa rõ.

Đo actual galley, CTA, icon, padding, close target và clip rect trong `sync_banner.rs` + container workspace. Dùng spacing/theme hiện hữu, budget theo content bounds; khi thiếu chỗ wrap/reflow hoặc thu gọn text phụ có chủ đích, giữ hành động dùng được. Không bắt buộc RTL, lề 12px hay thay nhãn thành “Đồng bộ” như một công thức. Không đổi chuỗi server để che reservation sai. Không sửa margin filter/thread row/timestamp nếu chưa tái hiện lỗi ở đó. Báo cáo mới về preview tệp cho thấy lỗi cắt chữ không chỉ ở banner; rà shared primitive nếu có, không suy hai lỗi cùng nguyên nhân.

Nghiệm thu: mọi trạng thái hỗ trợ, chuỗi cấu hình ngắn/dài/có dấu, hẹp/rộng, light/dark và DPI; CTA/đóng không cắt hoặc chồng vùng bấm, progress không làm nhảy sidebar; thành phần lân cận giữ bố cục đúng. So sánh vị trí/thứ bậc/nội dung từng trạng thái với PC có nguồn; chưa có nguồn ghi AMBIGUOUS.

### 4.2. Responsive và sidebar

Không bảo đảm timeline >=400 bằng ngưỡng cửa sổ tùy ý: `1025−64−340−340=281`; `720−64−300=356`, đều <400 trước padding. Tính budget gồm rail/sidebar/divider/drawer/timeline padding trước khi chọn inline/overlay/single-pane. Giữ hành vi compact/narrow đã có tới khi phương án mới qua kiểm chứng. Kích thước card voice 265/file 260 trong V1 là ứng viên, không minimum cứng ở panel nhỏ.

Rà filter tabs, preview sender/draft/typing/mention/media, badge, selected/hover, thread có tên dài và trạng thái chưa có dữ liệu. Đổi bố cục không đổi quy tắc unread/recall/privacy. Search inline và stranger inbox phải bảo toàn spec 006, hidden gates, FTS cancellation và exact identities; không tự chuyển stranger sang main chỉ vì reply nếu thiếu bằng chứng.

## 5. MP-CFG — Config, account settings và sync: ba loại sở hữu riêng

### 5.1. Inventory bắt buộc, gồm cả consumer

Mỗi trường có: tên/kiểu/đơn vị, remote policy hay preference, nguồn/method/envelope, thời điểm load, owner/runtime, default/validation/merge precedence, cache/persistence, consumer/hàm, lúc áp dụng, hot-update hay snapshot per operation, setter/readback nếu có, fixture/native evidence. Không dừng ở tải JSON rồi ghi file. Rà tất cả consumer và code hardcode cạnh tranh với config.

| Loại | Ví dụ | Quy tắc |
| :--- | :--- | :--- |
| Remote policy / feature gate | service map, cipher gate, zcinput, alias v2, file EECX, restricted extensions, CDN swap, sync origin gates/timeouts | Parse typed, xác minh precedence và consumer. Không coi là user-writable chỉ vì UI muốn toggle. |
| Account preference có hợp đồng | Các trường privacy được `src/privacy.rs` hỗ trợ | Giữ read-before-write, mutation một lần, ACK + readback và khả năng tài khoản thực. Mở rộng chỉ theo bằng chứng từng trường. |
| Local/device preference | Theme/font/scale, hotkey, folder tải, âm thanh local và các trường chưa có setter chứng minh | Giữ nơi lưu hiện hữu. Không gửi tất cả AppSettings lên server. Trường chưa rõ sở hữu ghi AMBIGUOUS. |

`src/privacy.rs` hiện dùng GET `/api/setting/me` và GET `/api/setting/update` qua helper typed. V1 đề xuất generic POST login/setting, setting/get, setting/update cho mọi tùy chọn **chưa có bằng chứng**; điều này không chứng minh các route đó không tồn tại hay không bao giờ nhận POST. Không thêm `UpdateServerSetting` tổng quát nếu chưa có schema/ownership/capability rõ.

### 5.2. Chuỗi startup và thời điểm áp dụng

Tái dùng bootstrap hiện hữu và [re-notes/73](../../re-notes/73-sync-origin-timeouts-and-startup-completion.md): auth/DB/cache bắt buộc → prerequisites có payload được UI áp trước Ready; optional preload settle/lỗi/skip và timeout được theo dõi riêng. Không dựng màn hình “đang đăng nhập” bằng sleep, không chặn vô hạn optional, không mở Ready trước prerequisites hoặc tự phát event completion.

Kiểm chứng khi restore session, QR/password login, reconnect, cache stale/lỗi, offline, account switch và relogin cùng UID. Ghi chính xác config nào load ở bootstrap/auth, cái nào refresh theo phiên/reconnect/control; chưa biết trigger không tự thêm polling. `getServerInfo` hiện có flattened signed GET bootstrap trong `src/login.rs`; không bọc lại mọi request theo một mẫu làm hỏng ngoại lệ này. Hosts vẫn resolve qua `endpoints.rs`/ServiceResolver; không hardcode domain/IP/gateway trong feature.

### 5.3. Các config phải truy đến consumer

- `zpw_service_map_v3`: resolve đúng logical service, refresh/fallback theo contract hiện có; không mặc định mọi route dùng chung host.
- `enable_encrypt_param`: rà gate và chuẩn ký/mã hóa theo từng đường có bằng chứng; không giả định bật/tắt đồng loạt mọi bootstrap request.
- `zcinput`: `inputColumnLayout` là flag 0/1; tỷ lệ expanded column 2/3 là khái niệm khác. Phân biệt legacy 5.5×22.5, default inputMaxLines 12.5 và budget text/tổng composer. Giữ fork deviation hiện hữu nếu chưa đủ dữ liệu rollout; không silently thay geometry.
- `enable_feature_alias_v2`: rà alias adapter, cache và consumer tên hiển thị; không quay về phonebook/pbName làm ghi đè alias khi gate mới có hiệu lực.
- `enable_file_eecx` + `restricted_ext_file`: rà parser/default/merge và điểm quyết định auto-ZIP trong worker. Giữ luồng đã sửa, không hardcode lại blacklist UI hoặc bỏ kiểm tra file đã đổi.
- `download_domains_swap`: chỉ thay domain hợp lệ theo policy, không đổi path/query tùy tiện hay log signed URL. URL mới và asset LRU đã có là hai trường hợp khác nhau; không flush toàn cache chỉ để ép swap. Kiểm chứng lỗi mạng/fallback/invalidation phù hợp, không mặc định đây là lỗi hiện tại.
- `sync_2_pc`: gate chung, SETTING, AUTO_SYNC, BANNER_SUGGEST_SYNC khác nhau. Theo re-notes/73, banner không phải SETTING; auto/manual timeout độc lập và snapshot theo operation. Legacy V1 chủ động giữ fork deviation; không auto-fallback hoặc tắt theo riêng gate Sync2.

### 5.4. Sửa nghĩa retry và bằng chứng tái kiểm tra

Artifact E2 PC: `compact-app-pc.0cd2e653e67efaa4bf4b.js`, SHA256 `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`. Bản lưu hiện đọc tại `%TEMP%/zalolite-direct-audit-20261122/pc/pc-dist/`; hash được kiểm tra lại khi sửa kế hoạch. Offset dưới đây là Unicode code point từ 0, không phải byte/UTF-16.

- `__waitAsyncFileInfo`, CP 8580062: fallback `upload_file_timeout_replay_last_chunk=[30,35,40,45,50,60]`; chọn `min(floor(totalSize/31457280),len−1)`, tức bucket 30 MiB. Đây không phải dãy backoff cho các lần retry chunk liên tiếp. Unit/time consumer phải được ghi trước khi áp dụng.
- Object `retryConfig` gần CP 8058000, trường `time_retry_default` CP 8058175: đã đọc lại các default `setting_feature:0`, `max_retry_count:2`, `time_retry_default:18e5` (=1800000). Consumer và override runtime vẫn phải xác minh khi triển khai. Không coi default là giá trị runtime mọi tài khoản; V1 ghi max=3 và đầu mảng=10 không được dùng làm contract.
- `inputColumnLayout` CP 6123457 được validator `.oneOf([0,1])`; geometry/rollout xem [re-notes/70](../../re-notes/70-composer-remote-config.md) và báo cáo nguồn trực tiếp. Không áp ratio 2/3 vào trường boolean.

Không thay retry mutation khi ACK không rõ; giữ clientId/idempotency và quy tắc error 204 đã sửa. Raw server `error_code=204` phải phân biệt HTTP 204; không dùng “204” chung để suy nguyên nhân.

### 5.5. UI Settings Sync

Ba thẻ của V1 chỉ là **đề xuất bố cục**, chưa được chứng minh là đúng nguyên bản PC. Thiết kế theo capability thật, dùng widget/theme chung và giữ toàn bộ entry Sync2, V1/Legacy, import/export/history tools còn hợp lệ. Không làm mất đường thao tác do gộp thành ba card.

- `auto_sync.enable` trong config hiện chứng minh gate origin AUTO_SYNC, không chứng minh preference có setter. Chưa có contract thì không tạo toggle ghi server/gate. Nếu muốn local opt-out, phải chốt ownership, precedence và mô tả fork deviation; không trình bày là bật được entitlement server.
- Sync điện thoại: trạng thái/progress/last-run/statistics có nguồn; Start/Stop/cancel dùng operation identity thực, late result không đổi tài khoản khác. Không thêm bước PIN hay authentication mới chỉ vì mockup.
- Usage zCloud không chứng minh API auto-backup/upload/backup snapshot. Chưa có hợp đồng thì không dựng toggle sao lưu, danh sách snapshot hoặc nút giả. Giữ local backup/import và media renewal riêng, không gắn nhãn thành cloud backup.

## 6. MP-UI — Chi tiết UX cần giữ và giới hạn triển khai

### 6.1. Sidebar/header/drawer

Quick filter/gán nhãn/alias edit có thể là popover bám anchor khi bằng chứng phù hợp; profile/settings/label management/PIN hoặc tác vụ nhiều bước có thể cần modal. Ngoài click/Escape/Save/Cancel phải nhất quán; trường chưa lưu không bị mất do đổi surface. Labels giữ parse/readback/race protections; số màu/giới hạn ký tự/sắp xếp phải kiểm tra dữ liệu và reference, không chốt từ bảng V1.

Header phân biệt 1-1/group/community và capability/role thật; community không mặc định luôn read-only. Presence phải qua privacy gate và có unknown state. Drawer có members/board/media/file/link/filter/back/multi-select và privacy actions theo task hiện hữu; không chốt số nút tuyệt đối hoặc TTL enum từ mockup. Tái dùng guard worker cho avatar/download/open/hidden chat; không thêm API chỉ để tạo icon/avatar.

### 6.2. Timeline, emoji, voice, file và rich cards

- Emoji renderer nếu cần: khảo sát assets/license/texture budget; test ZWJ, skin tone, cờ, VS15/16, dấu tổ hợp, selection/copy, wrapping, link/mention hitboxes, baseline và fallback. Không parse bằng một dải code point đơn giản hoặc sửa nội dung gốc khi vẽ.
- Voice: decode/cache/analyze ngoài UI; waveform thật nếu có samples, còn thiếu thì hiển thị trạng thái trung thực. Play/Pause/seek/duration/error liên kết playback đúng message; không dùng animation giả làm tiến độ. Phiên âm tách task theo quyền/contract/cost, không suy từ CMD 13394.
- File card: icon theo loại, tên/dung lượng/progress/retry/open/cancel theo trạng thái thật, đáp ứng chiều rộng nhỏ. Không ghi “đã lưu lâu dài” nếu retention không bảo đảm. Giữ kiểm tra owner/runtime/hidden/path/cache trong đường mở file hiện có; đánh giá execution policy theo loại tệp, không khẳng định source đã chứng minh lỗ hổng thực thi toàn bộ.
- Bubble/hover/reaction/system notice: đặt theo available rect, không tràn cạnh/che nút; mapping enum theo namespace. Payload unknown có fallback an toàn, không tự phân loại Recall thành system event.
- Poll/reminder/note/urgency: chỉ hiển thị nút thao tác khi end-to-end và quyền đã có bằng chứng; có loading/error/stale/readback. Không bịa phiếu/avatars/quyền hoặc dùng toast thay payload.

### 6.3. Composer: giữ đủ 15 ý định V1, rà từng capability

**Bố cục attachment đã được người dùng yêu cầu:** preview tệp chờ gửi phải ở **bên trong cùng khối composer**, cùng nền/container và content padding với toolbar, ô nhập/chú thích và nút Gửi; không dựng vùng preview rời trên timeline như ảnh fork hiện tại. Một thứ tự đề xuất để minh họa: toolbar → khay preview → ô nhập/chú thích + Gửi, toàn bộ nằm trong composer. Thứ tự/chi tiết visual cần đối chiếu trạng thái Zalo PC/Web tương ứng; đây không phải chốt kích thước từ mockup.

Xem [ba ảnh minh họa và giới hạn sử dụng](../assets/ui-ux-proposals-2026-11-24/README.md). Các ảnh chỉ minh họa nguyên tắc bố cục; khác biệt ngẫu nhiên về nav/nút/toolbar/card không tạo yêu cầu mới hoặc bằng chứng parity.

Khay preview có thumbnail/file icon, tên, dung lượng, nút bỏ từng tệp, thêm tệp và xóa tất cả theo trạng thái thật. Dành padding đủ ở cả hai cạnh, đo text/rect, không để tên bị clip trái hoặc cắt glyph; tên dài có ellipsis giữ nhận diện đuôi và cách xem tên đầy đủ. Nhiều tệp dùng overflow/scroll trong khay có giới hạn, không đẩy composer chiếm vô hạn timeline. Kiểm tra tên tiếng Việt dài, narrow/DPI, remove/add/caption/focus và đổi thread/account. Ảnh người dùng ngày 24/11 (`codex-clipboard-0e58c675-3f67-45bb-9074-574eae56a3b9.png`) là báo cáo bổ sung; không commit ảnh chứa dữ liệu tài khoản.

| # | Capability | Điều kiện triển khai / bảo toàn |
| :--- | :--- | :--- |
| 1 | Sticker | Picker/search/suggestion và animated formats theo decoder/nguồn thật; không thêm engine chỉ vì V1 nêu tên. |
| 2 | Ảnh/file/preview/drop/paste/caption | Caption đã có; giữ ADR-0029, path/dedup/budget/FileStamp và cách ly account/thread. Timeline drop gửi ngay và composer drop staging phải có hit-test/DPI chuẩn. |
| 3 | Danh thiếp | Chọn UID thật, payload adapter hiện có, ACK/unknown/error đúng. |
| 4 | Screenshot/annotation | Kiểm tra công cụ capture, quyền/shortcut, cancel/preview; không gửi tự động khi hủy. |
| 5 | Rich text | Rà config gate, styles/offset/schema hiện có; không thay wire từ mockup. |
| 6 | Tin nhanh `/` | Rõ local/account ownership, focus/IME, Escape, insert không vô tình gửi. |
| 7 | Thẻ ngân hàng | Xác minh format/contract hoặc ghi rõ local presentation; copy dùng dữ liệu thật. |
| 8 | Tạo poll | Nhóm/role/capability và contract adapter; tạo một lần, refresh/readback. |
| 9 | Nhắc hẹn | Server reminder khác local scheduled send; giữ identity/timezone/permission. |
| 10 | Ghi chú | Board linkage và owner permission thực, không suy từ creator UI. |
| 11 | Quan trọng | Chỉ dùng urgency enum được xác minh cho đúng namespace. |
| 12 | Khẩn cấp | Kiểm tra quyền/ACK, không mặc định tài khoản nào cũng dùng được. |
| 13 | Emoji/Like/Gửi | Trạng thái rỗng/có chữ/attachment/recording nhất quán, không mất draft hoặc gửi hai lần. |
| 14 | Link preview | Tái dùng worker có giới hạn fetch/cancel/privacy; không “cào” URL bất kỳ trên UI thread. |
| 15 | Mention/@All | UID/role/offset UTF-16 theo adapter, grapheme display riêng, không biến plain text thành mention ID. |

Auto-ZIP là phần bắt buộc bảo toàn ở capability 2: đọc gate/extensions hiện hành → kiểm tra file → đóng gói ngoài UI theo format có bằng chứng → upload/send/bubble qua đường hiện hữu. Không thêm giải nén tự động phía nhận từ giả thuyết. Test tên/đuôi nhiều chấm/hoa-thường/ZIP sẵn, file thay đổi/cancel/account switch/lỗi I/O/ACK không rõ. ZIP-08 native vẫn mở; không coi unit test hoặc một lần gửi ZIP thủ công là chứng minh mọi auto-ZIP đã đạt.

## 7. MP-NET — Các nghiên cứu tách khỏi sửa UI

1. **Metadata:** kiểm kê archive/unread/hidden/blocksharing/presence/mute và recent-search riêng. `removeUnreadMark` chỉ xử lý manual marker, không đồng nghĩa advance read cursor, clear mention hay gửi receipts hàng loạt. Với “Đánh dấu đã đọc”, xác minh từng operation/thứ tự/phạm vi và giữ menu khác còn dùng; không xóa chúng để khớp mockup.
2. **VoIP:** tái hiện trên hai thiết bị và phân biệt actor/caller/callee/recipient theo envelope thật; không nới điều kiện từ tên field. Receive, Miss, Reject, Cancel, Timeout, AnsweredElsewhere là những transition khác nhau. Watchdog 65s chỉ là local timeout, không chứng minh nhận ở nơi khác. Không dùng remote EndCall để dọn local ring của cuộc gọi nhận nơi khác.
3. Rà `GuiEvent::CallAnsweredElsewhere` trong `gui.rs`: source hiện gọi global stop_ringtone trước một số kiểm tra call matching, event chưa mang runtime. Ghi đây là rủi ro cần regression scoped event/call mới, **không** kết luận nguyên nhân lỗi popup hoặc lỗi đã tái hiện. Bảo toàn owner/runtime qua CallBridge/readiness, same-UID relogin và active/terminal ownership.
4. **E2EE:** `socket.rs` đã nhận diện 551/552; dispatch không chứng minh giải mã/interoperability. Giữ ADR-0035–0038 và trial opt-in, không plaintext fallback. Group sender keys/media encryption/receive pipeline chỉ mở theo evidence + task hiện hữu, không “bật CMD551” để tuyên bố hoàn tất.
5. **zCloud/media:** usage, media recovery, download renewal, upload backup và snapshot restore là các contract khác nhau. Làm theo spec 011; chưa chứng minh upload/snapshot thì để AMBIGUOUS. Không tự tạo API hay nối nút placeholder.
6. **SQLite:** không sao chép 15 shard của native chỉ để đạt UI parity. Nếu có nhu cầu hiệu năng thật, mở đề xuất riêng có đo đạc, ADR/migration/rollback, giữ WAL/FTS/cancellation/delete markers và dữ liệu người dùng.

## 8. Thứ tự triển khai và đầu ra mỗi giai đoạn

| Giai đoạn | Nhóm task | Đầu ra / điều kiện chuyển tiếp |
| :--- | :--- | :--- |
| 0 | MP-00A/B + shared popup/input | Baseline/capability & surface inventory; sửa hồi quy đa frame, native popup pass trước mở rộng UI phụ thuộc. |
| 1 | MP-01 banner/sidebar/layout | Sửa cắt chữ đúng phạm vi, budgets đo được, ảnh so sánh và input/state matrix; giữ origin/config hiện có. |
| 2 | MP-UI visual primitives | Icon/emoji/avatar/bubble/voice/file từng thay đổi nhỏ; assets/performance/semantics và native rendering được kiểm chứng. |
| 3 | MP-UI actions/composer/drawer/search/settings presentation | Dùng backend có sẵn, đầy đủ state/payload/permission; capability chưa sẵn không thêm nút giả. |
| 4 | MP-CFG consumer audit | Inventory field→load→store→consumer→effective state; sửa gap có evidence, cập nhật spec/verification hiện hữu. Có thể nghiên cứu song song pha 0–3. |
| 5 | MP-NET evidence-gated work | VoIP/E2EE/zCloud/server preference/recent-search riêng task/contract/fixture/native; chưa đủ bằng chứng tiếp tục độc lập, không đoán để đóng phase. |

Mỗi nhóm trước khi viết code ghi module/hàm tác động, source parity/deviation/unknown, guard cần giữ, regression và native steps trong task hiện hữu. Chia patch theo hành vi, không thay hàng nghìn dòng hoặc xóa guard chỉ vì V1 nêu khoảng dòng. Đổi source/test thì cập nhật baseline/hash và đánh giá lại phần phụ thuộc.

## 9. Bảo toàn các sửa đã audit và cổng nghiệm thu

Không làm mất `MetadataResultKind::{Snapshot,Read,Mutation}`, request identity/cutoff, producer revision/privacy watermark, owner/runtime fences, metadata admission, privacy gate, cache-before-Ready và monotonic merge. Không hồi sinh local delete/recall, không replay mutation/send khi ACK chưa rõ, giữ auto-ZIP/error204 và per-origin sync timeout. Optional preload, media renewal và metadata không được giành UI thread bằng I/O/crypto/ZIP.

Bằng chứng lịch sử trong dossier: candidate `c494c88d95052d3dfd8f1a8d9a9c13b6a184aa524962e197a606a08cbade17f3` đã qua 1154 library tests (11 ignored), 4 desktop-backup tests, clippy và stress 100 vòng/3000 lần chạy. Đây là kết quả cũ trong phạm vi remediation, không phải test vừa chạy cho kế hoạch, không chứng nhận mọi UI/server/native. ZIP-08, sync điện thoại và các device cases còn mở giữ nguyên trạng thái.

Mỗi patch mã chạy fmt/check, regression liên quan và clippy phù hợp; trước commit/push bắt buộc `scripts/audit-prepush.ps1` hoặc cargo check theo AGENTS. Các lệnh tham chiếu:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo clippy --locked --all-targets -j1 -- -D warnings
cargo test --locked --lib -j1
cargo test --locked --test desktop_backup_v2 -j1
powershell scripts/audit-prepush.ps1
```

Chọn regression thực theo patch; khi ảnh hưởng shared worker/protocol/cache thì chạy suite rộng. Không viết test chỉ lặp implementation hoặc chạy test đơn luồng để che race. Chỉ ghi PASS cho lệnh thực chạy trên fingerprint nêu rõ.

Nghiệm thu UI theo [quy chế chung](../standards/ui-ux-consistency.md): chuỗi tiếng Việt ngắn/dài/config động, min/normal/wide + sidebar resize + drawer, light/dark, DPI 100/125/150/200%, pointer/keyboard/IME/focus, overlay nhiều frame và account transitions khi liên quan. Trường hợp không chạy được ghi pending cụ thể; ảnh chụp cần che dữ liệu. Chỉ thao tác gửi/gọi/đổi tài khoản thật khi đã có ủy quyền tương ứng.

Nếu yêu cầu hai lượt audit sạch, cả hai phải cùng source/test fingerprint, khác góc rà soát và có bằng chứng; finding mới hoặc sửa nguồn/test làm bộ đếm về 0. Nêu đúng ai review, không gọi hai phương pháp của một agent là hai reviewer độc lập. Native acceptance không được tự đóng từ hai audit offline.

## 10. Phản hồi triển khai cho Antigravity

Prompt bàn giao đầy đủ, quy trình patch/kiểm chứng và mẫu hồ sơ Codex audit: [hướng dẫn thực thi Antigravity](2026-11-24-antigravity-execution-prompt.md). File này điều phối V2, không tạo bộ policy hoặc trạng thái nghiệm thu riêng.

Dùng V2 này làm kế hoạch repo chuẩn, thay các giả định V1 tương ứng trong bản brain bạn đang dùng; không để hai bản chỉ dẫn trái nhau. Trước patch đầu tiên, xuất bảng: mục V1 → quyết định V2 → code hiện có → gap → evidence → task/spec → kiểm chứng. Giữ đủ 17 mục và 15 capability, không đánh dấu done vì bảng đã được viết.

Bắt đầu MP-00 trên baseline thực, sau đó MP-01; ghi kế hoạch patch ngắn có các guard cần giữ trước khi sửa. Tiếp tục các task độc lập đã được giao; với contract thiếu bằng chứng, ghi AMBIGUOUS và thu thập nguồn thay vì dựng endpoint/enum/payload. Không đợi hỏi lại cho lựa chọn thường lệ đã được quy chế giải quyết. Không tự coi tài liệu này là quyền commit/push/release hay quyền tác động tài khoản ngoài phạm vi phiên làm việc.

Báo cáo sau mỗi patch: thay gì/vì sao, source và file ảnh hưởng, checks thực chạy, native cases đã chạy/còn chờ, rủi ro/rollback. Dùng trạng thái trong spec/verification hiện hữu; completion chỉ khi hành vi, UI/UX và luồng dữ liệu cùng đạt tiêu chí.
