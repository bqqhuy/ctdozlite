# ĐẶC TẢ NGHIÊN CỨU THỰC NGHIỆM: CƠ CHẾ PRELOAD, VÒNG ĐỜI DỮ LIỆU, MEDIASTORE RIGHT PANEL, LAZYLOAD VÀ GIAO DIỆN MẶC ĐỊNH (ZALO PARITY SPECIFICATION)

- **Mã tài liệu:** `SPEC-2026-11-PRELOAD-MEDIASTORE-LAZYLOAD-PARITY`
- **Phiên bản:** `2.0.0` (Cập nhật phản biện mã nguồn Zalo PC 26.11.12, Zalo Web và kiểm kê toàn diện codebase `ctdozlite`)
- **Ngày hoàn thiện:** 25/11/2026 (Asia/Saigon)
- **Đối tượng áp dụng:** Kiến trúc sư phần mềm, AI Orchestrator, Reviewer độc lập (Alpha/Beta) và kỹ sư triển khai `ctdozlite`.
- **Mục tiêu:** Cung cấp hồ sơ phân tích thực nghiệm chuẩn xác, bằng chứng mã nguồn tĩnh (E2) từ Zalo PC 26.11.12 (`sync-v2-sub-worker`, `compact-app-pc`) và Zalo Web (`web-bundle/default-embed-render.js`), đối chiếu bản đồ tái sử dụng module hiện có trong fork, sửa đổi toàn diện các kết luận suy đoán quá mức của bản thảo trước, và xác lập kế hoạch triển khai phân kỳ khả thi, có thể kiểm chứng độc lập.
- **Phạm vi tài liệu:** **Chỉ nghiên cứu, kiểm chứng và lập tài liệu kỹ thuật.** Tuyệt đối không can thiệp mã nguồn `src/`, không commit/push/release và không tự ý chuyển sang lập trình khi chưa có phê duyệt từ Codex audit.
- **Nguyên tắc quản trị:** Tuân thủ tuyệt đối `AGENTS.md`, `docs/ZALO_REFERENCE_ANALYSIS.md`, các tiêu chuẩn kỹ thuật `docs/standards/` và kỷ luật Karpathy:
  1. **Bằng chứng trước thiết kế:** Phân loại rõ ràng nguồn dữ liệu: `E1` (Live Capture), `E2` (Static Bundle Read), `E3 / AMBIGUOUS` (Suy luận kỹ thuật), `CODE` (Kiểm chứng unit test mã nội bộ), `FIXTURE` (Replay dữ liệu mẫu), `NATIVE/DEVICE` (Chạy thực tế trên thiết bị vật lý).
  2. **Tôn trọng hiện trạng và tối đa hóa tái sử dụng:** Khảo sát thấu đáo các module đã có (`src/server_media.rs`, `src/gui/worker.rs`, `src/gui/tools_panel.rs`, `src/endpoints.rs`, `src/history_open.rs`, `src/store.rs`, `src/zcloud.rs`). Tuyệt đối không tạo lại subsystem trùng lặp.
  3. **Bảo toàn bất biến phiên và an toàn khởi động:** Không nới lỏng cơ chế kiểm tra phiên, fencing runtime/account, và xử lý mã lỗi máy chủ (`600`, `-102`, `401`).

---

## MỤC LỤC
1. [TUYÊN BỐ HIỆU CHỈNH & PHÂN LOẠI BẰNG CHỨNG (RETRACTION & EVIDENCE CLASSIFICATION)](#1-tuyên-bố-hiệu-chỉnh--phân-loại-bằng-chứng-retraction--evidence-classification)
2. [SỔ BẰNG CHỨNG THỰC NGHIỆM CHI TIẾT (EVIDENCE LEDGER)](#2-sổ-bằng-chứng-thực-nghiệm-chi-tiết-evidence-ledger)
3. [BẢN ĐỒ TÁI SỬ DỤNG MÃ NGUỒN FORK HIỆN TẠI (EXISTING CODEBASE REUSE MAP)](#3-bản-đồ-tái-sử-dụng-mã-nguồn-fork-hiện-tại-existing-codebase-reuse-map)
4. [HỢP ĐỒNG KHỞI ĐỘNG & MA TRẬN SẴN SÀNG (STARTUP READINESS & DATA LIFECYCLE)](#4-hợp-đồng-khởi-động--ma-trận-sẵn-sàng-startup-readiness--data-lifecycle)
5. [KIẾN TRÚC MEDIASTORE & RIGHT PANEL DRAWER (INDEPENDENT MEDIA VAULT)](#5-kiến-trúc-mediastore--right-panel-drawer-independent-media-vault)
6. [CƠ CHẾ LAZYLOAD / INFINITE SCROLL & NEO VỊ TRÍ CUỘN (SCROLL ANCHORING)](#6-cơ-chế-lazyload--infinite-scroll--neo-vị-trí-cuộn-scroll-anchoring)
7. [MÀN HÌNH CHÀO MỪNG (WELCOME ONBOARDING) & GIAO DIỆN MẶC ĐỊNH SÁNG (LIGHT THEME)](#7-màn-hình-chào-mừng-welcome-onboarding--giao-diện-mặc-định-sáng-light-theme)
8. [LỘ TRÌNH TRIỂN KHAI PHÂN KỲ (PHASED IMPLEMENTATION PLAN)](#8-lộ-trình-triển-khai-phân-kỳ-phased-implementation-plan)
9. [MA TRẬN KIỂM CHỨNG, NGHIỆM THU KÉP & CÂU HỎI PHẢN BIỆN (VERIFICATION & CODEX CRITIQUE)](#9-ma-trận-kiểm-chứng-nghiệm-thu-kép--câu-hỏi-phản-biện-verification--codex-critique)

---

## 1. TUYÊN BỐ HIỆU CHỈNH & PHÂN LOẠI BẰNG CHỨNG (RETRACTION & EVIDENCE CLASSIFICATION)

Bản thảo sơ bộ ngày 25/11/2026 có một số nhận định dựa trên suy diễn chưa được kiểm chứng đầy đủ hoặc mô tả thiếu chính xác so với mã nguồn thực tế của Zalo PC. Bản đặc tả v2.0 này chính thức đính chính và làm rõ:

### 1.1. Các Điểm Đính Chính Cốt Lõi
1. **Về Danh bạ và Khởi động (`FriendsManager`):**
   - *Bản thảo cũ viết:* "Khởi động chỉ đọc SQLite, không bao giờ gọi mạng".
   - *Hiệu chỉnh chuẩn xác (E2):* `FriendsManager._startFriendManager()` đọc cache cục bộ thông qua `_loadFriendLocal()`. Nếu cache rỗng (`"No data"`), số lượng quá ít (`count < 10`) hoặc bị lỗi, Zalo PC **vẫn gọi mạng ngay lập tức** bằng `_request("NO_DATA")`. Nó chỉ trì hoãn bằng hẹn giờ ngầm (`_setTimerRequest`) khi cache cục bộ hợp lệ và thời gian thành công gần nhất chưa vượt quá TTL 36 giờ (`time_interval_fetch_fr_list: 1296e5` ms).
   - *Thông số `count: 20000`:* Đây là tham số kích thước trang yêu cầu tối đa của RPC (`count: 20000`), **không phải là cam kết máy chủ luôn trả về 20.000 bạn bè**. Không tự ý biến điều kiện `count < 10` của Zalo PC thành heuristic cứng nhắc trong fork nếu tài khoản người dùng thực sự có ít hơn 10 bạn bè.
2. **Về Cơ Chế Preload Tin Nhắn (`max_preload_msg: 40`):**
   - *Bản thảo cũ viết:* "max_preload_msg: 40 nghĩa là server chỉ giữ 40 tin nhắn text trong timeline hoặc giới hạn lưu trữ server".
   - *Hiệu chỉnh chuẩn xác (E2):* `max_preload_msg: 40` là **kích thước bộ đệm con trỏ (cache cursor map size)** mà hàm `getLocalMsgIdForPreload()` thu thập từ `this.cache` (tối đa 40 cặp `{ threadId_0: msgId, groupId_1: msgId }`). Khi đăng nhập mới (cache rỗng), nó gửi map rỗng `{}` để server trả về tin nhắn mới nhất của các hội thoại gần đây. Khi khởi động ấm (warm cache), nó gửi map con trỏ để lấy phần chênh lệch (delta). Đây hoàn toàn không phải giới hạn tổng số tin nhắn lưu trên server.
   - *Về thời gian khởi động:* Thu hồi các con số phỏng đoán chủ quan ("300-800ms", "fork 3-8s") và không đưa ra tiêu chuẩn cứng ngắc `< 800ms` khi chưa có benchmark thực nghiệm trên phần cứng ARM64 mục tiêu.
3. **Về Giao Thức MediaStore (`getMediaStorePreview`):**
   - *Bản thảo cũ viết:* "Gửi POST với body là JSON mã hóa AES".
   - *Hiệu chỉnh chuẩn xác (E2):* Mã nguồn `compact-app-pc.js:15052218` chỉ rõ:
     `const c = l + n + this._getCommonParams() + "&params=" + encodeAES(JSON.stringify(o)); return this._post(c, null, null, a);`
     Nghĩa là: **Tham số mã hóa `params` nằm hoàn toàn trong QUERY STRING của URL, còn HTTP POST Body là `null` (rỗng)**!
   - *Payload send2me vs Group:* Tuyệt đối không thêm `group_id` hay `imei` vào payload của `send2me` vì mã nguồn gốc chỉ gửi `{ limit: 50 }`. Với nhóm thông thường, payload mới chứa `{ group_id, imei, limit: 50 }`.
   - *Mã định danh lệnh (CmdId):* Các mã `11843`, `12221`, `11841`, `12220` là **HTTP RPC Wrapper CmdId** phục vụ telemetry và bộ điều phối nội bộ của Zalo PC, **không phải WebSocket Frame Command ID**.
4. **Về "Cloud Của Tôi" (send2me) & zCloud:**
   - *Bản thảo cũ viết:* "Dữ liệu tồn tại vĩnh viễn trên server, luôn có sẵn mọi file cũ".
   - *Hiệu chỉnh chuẩn xác (E2/E3):* Tách biệt 5 khái niệm:
     - *Hội thoại Send2me:* Định danh tài khoản cá nhân (`s.login.send2me_id`).
     - *Chỉ mục MediaStore:* Danh mục metadata các file/ảnh đã gửi mà server theo dõi.
     - *Tài nguyên CDN thực tế:* File nhị phân lưu tại các CDN (`dlmd.me`, `dlfl.vn`, `zdn.vn`). File có thể bị hết hạn (`expired: true`, HTTP 404/410) nếu vượt quá chính sách lưu trữ tạm thời của Zalo hoặc tài khoản không có gói zCloud hoạt động.
     - *Quyền lợi zCloud:* Gói dịch vụ mở rộng dung lượng và thời gian lưu trữ dài hạn.
     - *Bộ đệm tin nhắn máy chủ:* Không kết luận server chỉ lưu "30-50 tin rolling buffer" một cách tuyệt đối khi chưa có bằng chứng phân tích phạm vi toàn bộ hệ thống lưu trữ backend Zalo.

### 1.2. Hệ Thống Phân Loại Bằng Chứng
Mọi kết luận trong tài liệu này được dán nhãn theo chuẩn `AGENTS.md` & `docs/ZALO_REFERENCE_ANALYSIS.md`:
- **[E1] Live Network Capture:** Bắt gói trực tiếp qua proxy/wireshark trên phiên Zalo PC / Web thật.
- **[E2] Static Bundle Decompile:** Đọc trực tiếp AST/mã nguồn dịch ngược từ Zalo PC 26.11.12 hoặc Zalo Web bundle.
- **[E3 / AMBIGUOUS] Inference / Ambiguity:** Suy luận kỹ thuật từ giao thức lân cận hoặc hành vi quan sát, cần xác minh thêm.
- **[CODE] Codebase Inspection:** Đối chiếu trực tiếp mã Rust hiện có trong `ctdozlite` (`src/`).
- **[FIXTURE] Mock / Recorded Test:** Kiểm thử tự động trên bộ dữ liệu giả lập hoặc payload ghi lại.
- **[NATIVE/DEVICE] Device Verification:** Kiểm chứng chạy thật trên Windows 11 ARM64 với tài khoản thật.

---

## 2. SỔ BẰNG CHỨNG THỰC NGHIỆM CHI TIẾT (EVIDENCE LEDGER)

Dưới đây là bảng trích xuất bằng chứng mã nguồn tĩnh từ bản phân phối chính thức của Zalo PC và Zalo Web:

| Mã Bằng Chứng | Nguồn / File / Module | Vị Trí Cụ Thể (Offset / Line) | Bằng Chứng Mã Nguồn & Ý Nghĩa Kỹ Thuật |
| :---: | :--- | :--- | :--- |
| **E2-PRELOAD-01** | `Zalo-26.11.12/resources/app.asar`<br>`sync-v2-sub-worker.0cd2e653e67efaa4bf4b.js` | Offset UTF-16: `14262761`<br>Hàm: `async _doPreloadData()` | **State Machine 4 bước:** `[Login, PreloadData, Ready, LaunchIdle]`.<br>Trình tự gọi tuần tự có kiểm soát:<br>`await FriendsManager.init();`<br>`await GroupsManager.init();`<br>`await PreviewDataManager.init();`<br>`let e = C.ModuleContainer.resolve(a.Application); await e.init();`<br>`recordEvent(MetricEvent.app_done_preload_data); this._next();` |
| **E2-FRIEND-01** | `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Module `pOC5`, `UiPd` | Offset: `17967866`<br>Hàm: `_loadFriendLocal()` & `_initTimeGetFriendList()` | **Chiến lược đọc danh bạ & TTL 36h:**<br>`time_interval_fetch_fr_list: 1296e5` ms (36 giờ).<br>`_loadFriendLocal()` gọi `o.default.getFriends(null, !1)` đọc từ IndexedDB/SQLite cục bộ. Nếu có dữ liệu (`n.length > 10`), trả về ngay. Nếu không có hoặc lỗi, reject `"No data"` dẫn tới `_request("NO_DATA")`.<br>Nếu cache còn hạn, tính `Math.abs(Date.now() - this._getLastTimeFetchFriendSuccess()) - 1296e5` và đặt timer ngầm `_setTimerRequest(e)`. |
| **E2-PREVIEW-01** | `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Module `LastMsgPreprocessor`, `yzMR` | Offset: `833929`, `4260864`<br>Hàm: `getLocalMsgIdForPreload()` | **Cơ chế thu thập con trỏ preview:**<br>`get maxSize() { return s.default.max_preload_msg; }` (`max_preload_msg = 40`).<br>Lấy tối đa 40 hội thoại từ `this.cache`, tạo map `{ [id_0 hoặc id_1]: msgId }`. Gửi map này qua `getLastMessagesForPreview`. Khi cache rỗng, map là `{}`. |
| **E2-SETTLE-01** | `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Module `JhTw`, `offline2Online` | Offset: `6405237`, `6983340`<br>Hàm: `onEndPreProcessing()` | **Cơ chế gỡ bỏ loading indicator:**<br>Khi `getLastMessagesForPreview` hoàn tất hoặc bỏ qua, kích hoạt `PreProcessingSettle` -> gọi `onEndPreProcessing()` -> chuyển sang `_doReady()` -> gọi `Application.ready()` -> phát `app_ready` -> thực thi `doRemoveLoadingIndicator()`. |
| **E2-WATCHDOG-01**| `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Module `LastMsgPreprocessor` & `Ready` | Offset: `834102`, `14263100` | **Hệ thống Watchdog Timeout:**<br>`get_last_message_timeout_before_polling: 1e4` (10s watchdog cho preview).<br>`_timeoutStepReady: 1e4` (10s watchdog cho bước Ready).<br>`loading_timeout: 3e4` (30s hard limit bảo vệ toàn bộ ứng dụng). |
| **E2-MEDIA-01** | `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Component MediaStore | Offset: `15052218`<br>Hàm: `getMediaStorePreview()` | **Hợp đồng MediaStore Preview (CmdId 11843 / 12221):**<br>`POST` với Query String chứa `params=encodeAES(...)`, body là `null`.<br>Send2me (`e === Config.sendToMeId`): Endpoint `/api/media/oneone/preview?`, domain `media_store_send2me`, CmdId `12221`, payload `{ limit: 50 }`.<br>Nhóm/Direct: Endpoint `/api/mediastore/preview?`, domain `media_store`, CmdId `11843`, payload `{ group_id, imei, limit: 50 }`. |
| **E2-MEDIA-02** | `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Component MediaStore | Offset: `15052450`<br>Hàm: `getMediaFromConversation()` | **Hợp đồng MediaStore List (CmdId 11841 / 12220):**<br>`GET` với Query String chứa `params=encodeAES(...)`.<br>Send2me: Endpoint `/api/media/oneone/list`, CmdId `12220`.<br>Nhóm/Direct: Endpoint `/api/mediastore/list`, CmdId `11841`.<br>Payload: `{ media_type, limit, last_id, ... }`. Ánh xạ `media_type`: `"media"` (ảnh/video), `"file"`, `"link"`. |
| **E2-LAZYLOAD-01**| `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Message View Container | Offset: `569225`, `443753`<br>Hàm: `onScroll()` | **Cơ chế Infinite Scroll & Neo cuộn:**<br>Bắt sự kiện: `"up" === scrollDirection && isNearTop`.<br>Kích hoạt `loadMoreMessages()` -> bật cờ `this._isScrollKeep = true` (khóa vị trí đọc) -> hiển thị `SHOW_LOAD_MORE_TOP_INDICATOR` (spinner đỉnh). Khi nhận tin nhắn mới chèn vào đầu: tính lại vị trí cuộn và đặt `this._isScrollKeep = false`. |
| **E2-ONBOARD-01** | `pc-dist/compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Module `/KU8`, Component `P` | Offset: `1127631`, `1805597`<br>Template: `chat-onboard` | **Welcome Onboarding Screen:**<br>Hiển thị ở cột phải khi `selectedConvId == null`.<br>Tiêu đề: `STR_WELCOME_SCREEN_MAIN_TITLE` ("Chào mừng bạn đến với Zalo PC!").<br>Carousel 4 slide giới thiệu tính năng kèm dot pagination. |
| **E2-WEB-01** | `web-bundle/default-embed-render.js` | Khóa cấu hình | Xác nhận parity cấu hình trên Zalo Web: `time_interval_fetch_fr_list: 1296e5` ms, `time_interval_verify_fr_list: 864e5` ms. |

---

## 3. BẢN ĐỒ TÁI SỬ DỤNG MÃ NGUỒN FORK HIỆN TẠI (EXISTING CODEBASE REUSE MAP)

Kiểm tra trực tiếp mã nguồn `ctdozlite` (`src/`) cho thấy phần lớn hạ tầng giao thức và xử lý dữ liệu đã được xây dựng nhưng đang bị phân mảnh, chưa được kết nối vào giao diện hoặc đang bị dùng sai mục đích:

| Khả Năng / Subsystem | Vị Trí Đã Có Trong Fork | Caller Hiện Tại Trong UI | Hiện Trạng Thiếu Sót & Sai Lệch | Bằng Chứng Mã Nguồn | Đề Xuất Thay Đổi Tối Thiểu (Minimal Change) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Kho Media Server (MediaStore Client)** | `src/server_media.rs`<br>`fetch_conversation_media_store`<br>`ServerMediaType`<br>`ServerMediaItem`<br>`ServerMediaPage` | `src/gui/tools_panel.rs` (Bảng công cụ phụ trợ: 📦 Kho Lưu Trữ Media Server & zCloud) | Đã có sẵn toàn bộ hàm gọi API `/api/mediastore/list` và `/api/media/oneone/list`, phân trang qua `last_id`, xử lý send2me và domain resolver. Tuy nhiên, **Right Drawer (`src/gui/workspace.rs:11063`) không hề sử dụng**, mà lại đi duyệt mảng `thread.messages` và `archive_history.messages` cục bộ! | `src/server_media.rs:284`<br>`src/gui/workspace.rs:11063` | **Tái sử dụng 100% `src/server_media.rs`** cho Right Drawer. Bổ sung endpoint `/preview` hoặc tái sử dụng `/list` với `limit: 50` để nạp kho media khi mở Drawer. Không viết mới module media trong `chat.rs`! |
| **Lệnh Worker MediaStore** | `src/gui/worker.rs:758`<br>`WorkerCmd::FetchServerMedia`<br>`src/gui.rs:871`<br>`GuiEvent::ServerMediaLoaded`<br>`GuiEvent::ServerMediaLoadFailed` | `src/gui/tools_panel.rs:920, 947, 974` | Đã có đầy đủ lệnh IPC worker và event trả về cho GUI, hỗ trợ `runtime_id`, `thread_id`, `media_type`, `cursor`, `generation`. Nhưng sự kiện `GuiEvent::ServerMediaLoaded` chỉ cập nhật vào `self.tools_panel.server_media_items`, không chuyển dữ liệu vào state của Workspace Right Drawer. | `src/gui/worker.rs:758`<br>`src/gui.rs:7302` | Mở rộng luồng xử lý `GuiEvent::ServerMediaLoaded` trong `src/gui.rs` để cập nhật trực tiếp vào bộ nhớ đệm `ConversationMediaState` của Right Drawer theo từng thread và từng category. |
| **Tải Lịch Sử Hội Thoại (Group/Direct History)** | `src/gui/worker.rs:541`<br>`WorkerCmd::LoadGroupHistory`<br>`src/history_open.rs`<br>`crate::history_open::fetch` | `src/gui/workspace.rs:1447, 4003, 6033`<br>Nút thủ công: "Tải thêm tin nhắn cũ" | `WorkerCmd::LoadGroupHistory { group_id, is_group, limit }` **chưa hỗ trợ tham số `before_msg_id`**. Lệnh này hiện tại chỉ gọi `store.hydrate_selected_threads` (đọc SQLite cục bộ) rồi gọi `history_open::fetch` lấy head lịch sử server. Chưa hỗ trợ phân trang lùi thời gian cho infinite scroll. | `src/gui/worker.rs:541, 5833`<br>`src/gui/workspace.rs:6033` | Bổ sung trường `before_msg_id: Option<String>` vào `WorkerCmd::LoadGroupHistory` (hoặc tái sử dụng `WorkerCmd::LoadArchiveHistory` vốn đã có con trỏ `before: Option<HistoryCursor>`) để hỗ trợ lazyload phân tầng: đọc tiếp SQLite cục bộ trước, khi chạm đáy local mới gọi server history. |
| **Con Trỏ Preview Tin Nhắn (`get_last_msgs`)** | `src/chat.rs:5165`<br>`chat::get_last_msgs` | `src/gui/worker.rs:2387`<br>(Khởi động preload) | Hàm `chat::get_last_msgs` nhận `thread_ids: &[String]`. Khi rỗng (`&[]`), nó gửi `threadIdLocalMsgId: "{}"`. Nhưng khi có danh sách ID, nó gán cứng con trỏ `json!("1")` (`map.insert(key, json!("1"))`), thay vì truyền `msgId` thực tế từ cache cục bộ như Zalo PC (`getLocalMsgIdForPreload`). | `src/chat.rs:5180`<br>`src/gui/worker.rs:2387` | Cho phép truyền map thực tế `&[(&str, &str)]` (id và last_msg_id) từ SQLite vào `get_last_msgs`. Khi cache rỗng, tiếp tục truyền `{}` để lấy tin mới nhất. |
| **Bộ Phân Giải Tên Miền (Service Resolver)** | `src/endpoints.rs`<br>`src/service_resolver.rs` | `src/server_media.rs:328` | Đã có sẵn định nghĩa `media_store` và `media_store_send2me` trong `ServiceResolver`. `endpoints.rs` đã chứa bảng domain và cơ chế fallbacks an toàn. | `src/service_resolver.rs:21`<br>`src/endpoints.rs:10` | Tái sử dụng nguyên vẹn resolver hiện có. Tuyệt đối không hardcode URL `*.zaloapp.com` hay `*.chat.zalo.me`. |
| **Quản Lý Trạng Thái Chọn Hội Thoại (Active Thread)** | `src/gui.rs:1515`<br>`selected_thread_index: usize` | Rải rác khắp `src/gui.rs`, `src/gui/workspace.rs`, `src/gui/modals.rs` | `selected_thread_index` là kiểu số nguyên nguyên thủy (`usize`). Mặc định khởi tạo là `0`. Khi đăng nhập xong, app luôn mở ngay hội thoại tại index 0 (vốn bị gán cứng là "Cloud của tôi"), triệt tiêu hoàn toàn trạng thái rỗng và Welcome Onboarding Screen. | `src/gui.rs:1515`<br>`src/gui.rs:1874`<br>`src/gui/worker.rs:2508` | Chuyển đổi an toàn: Sử dụng `active_thread_id: Option<String>` song song với kiểm tra index. Khi khởi động vào `Workspace`, nếu chưa có tương tác chọn chat, giữ `active_thread_id = None` và render Welcome Screen. |
| **Chế Độ Giao Diện Mặc Định (Theme Mode)** | `src/gui.rs:1095, 1166`<br>`Settings::theme_mode` | `src/gui.rs:1831, 7704` | `Settings::default()` đang gán `theme_mode: "dark".into()`, và fallback match cũng chọn `dark`. Điều này ngược với giao diện mặc định của Zalo PC và Zalo Web (luôn là giao diện Sáng / Light). | `src/gui.rs:1166, 1834` | Sửa giá trị mặc định của `theme_mode` thành `"light".into()` trong `Settings::default()` và fallback, đảm bảo người dùng mới mở app nhận được giao diện Sáng chuẩn mực Zalo, trong khi vẫn tôn trọng cài đặt người dùng đã lưu. |

---

## 4. HỢP ĐỒNG KHỞI ĐỘNG & MA TRẬN SẴN SÀNG (STARTUP READINESS & DATA LIFECYCLE)

Nhằm giải quyết triệt để vấn đề thời gian chờ đợi lâu sau khi đăng nhập mà không làm gãy các bất biến bảo mật và tính toàn vẹn dữ liệu, tiến trình khởi động được chuẩn hóa thành ma trận điều phối rõ ràng.

### 4.1. Phân Tách Nhiệm Vụ Bắt Buộc vs Tác Vụ Chạy Ngầm
- **Điều kiện tiên quyết bắt buộc (Mandatory Prerequisites - Không được bỏ qua):**
  1. *Khởi tạo Cơ sở dữ liệu SQLite cục bộ:* Mở file DB tài khoản, chạy migration kiểm tra schema (`store.open_for_account_write`).
  2. *Xác thực phiên (Session Validation):* Kiểm tra token/cookies và session keys. Nếu phiên hết hạn (`401`, `-102`, `600`), lập tức chuyển về màn hình đăng nhập; **Watchdog tuyệt đối không được bypass lỗi xác thực**.
  3. *Tải cấu hình E2EE cục bộ:* Khởi tạo khóa mã hóa Signal cục bộ từ store.
- **Tác vụ lấy dữ liệu khởi đầu (Initial Baseline Fetch - Có Watchdog bảo vệ):**
  1. *Nạp bộ đệm danh bạ (Friends Baseline):* Kiểm tra cache SQLite. Nếu hợp lệ và chưa quá 36 giờ -> nạp ngay từ local. Nếu rỗng/hỏng -> gọi mạng với timeout 4s.
  2. *Nạp xem trước tin nhắn (Preview Baseline):* Gọi `chat::get_last_msgs` với timeout 4s.
- **Tác vụ chạy ngầm khi rảnh (Idle Background Tasks - Tách rời hoàn toàn khỏi UI thread):**
  1. *Tải danh bạ đầy đủ (Full Friends Sync):* Hẹn giờ fetch 36h chạy ngầm, không chặn hiển thị màn hình chính.
  2. *Enrichment chi tiết nhóm (Group Profile Enrichment):* Tải tên/avatar thành viên khi người dùng cuộn tới.
  3. *Đồng bộ lịch sử cũ (Archive / Phone Sync):* Chạy qua tiến trình riêng biệt khi người dùng yêu cầu.

### 4.2. Ma Trận Xử Lý Các Kịch Bản Khởi Động

| Kịch Bản Khởi Động | Trạng Thái Cache Local | Hành Vi Danh Bạ (`FriendsManager`) | Hành Vi Tin Nhắn (`LastMsgPreprocessor`) | Điều Kiện Mở Khung Làm Việc (Workspace Unblock) | Trạng Thái Hiển Thị (UI State) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **1. Fresh Login (Đăng nhập mới tinh)** | Rỗng hoàn toàn | Gọi mạng `_request("NO_DATA")` lấy danh bạ. Timeout 4s. | Gửi `get_last_msgs` với map rỗng `{}` lấy preview các hội thoại gần đây. Timeout 4s. | Ngay khi có preview tin nhắn HOẶC timeout 4s chạm ngưỡng. | Mở Workspace, hiển thị danh sách hội thoại bên trái, bên phải là **Welcome Onboarding Screen** (chưa chọn chat nào). |
| **2. Warm Start (Khởi động ấm - Cache còn hạn < 36h)** | Đầy đủ, hợp lệ, last_sync < 36h | **Chỉ đọc SQLite cục bộ**, không gọi mạng. Đặt timer ngầm `_setTimerRequest`. | Gửi `get_last_msgs` kèm map con trỏ `{ id: last_msg_id }` lấy delta tin mới. Timeout 3s. | Ngay khi nạp xong cache SQLite và gửi xong request delta (thường < 500ms). | Mở Workspace ngay lập tức. Dữ liệu tin nhắn mới cập nhật mượt mà. |
| **3. Warm Start (Cache quá hạn > 36h)** | Có dữ liệu, nhưng last_sync > 36h | Đọc SQLite hiển thị ngay; kích hoạt task ngầm fetch cập nhật danh bạ mà không chặn UI. | Gửi `get_last_msgs` lấy delta tin mới. | Mở Workspace ngay sau khi đọc xong SQLite cục bộ. | Không bị chặn màn hình loading. Danh bạ tự cập nhật ngầm khi task hoàn tất. |
| **4. Cache Hỏng / Bị Lỗi (Corrupt / Incomplete)** | Đọc SQLite trả về lỗi hoặc dữ liệu không nhất quán | Đánh dấu cache invalid, gọi mạng fallback `_request("NO_DATA")`. | Gửi `get_last_msgs` map rỗng để phục hồi trạng thái preview. | Hoàn tất mạng fallback HOẶC hết watchdog timeout 4s. | Workspace mở ở chế độ Partial, thanh trạng thái hiển thị thông báo nạp phục hồi. |
| **5. Mất Kết Nối Mạng (Offline Startup)** | Có cache cục bộ từ trước | Đọc toàn bộ từ SQLite cục bộ. Bỏ qua các lệnh fetch mạng. | Đọc tin nhắn gần nhất từ SQLite cục bộ. | Mở Workspace ngay lập tức từ dữ liệu offline. | Workspace hiển thị biểu tượng Offline/Mất kết nối trên đỉnh, toàn bộ lịch sử đã lưu vẫn đọc bình thường. |
| **6. Chuyển Đổi Tài Khoản (Account Switch)** | Khác định danh tài khoản trước | Đóng toàn bộ kết nối và store tài khoản cũ. Mở store tài khoản mới (Account fencing). | Nạp baseline riêng biệt của tài khoản mới. | Mở Workspace của tài khoản mới khi hoàn tất nạp store tương ứng. | Ngăn chặn triệt để rò rỉ dữ liệu chéo giữa các tài khoản. |

---

## 5. KIẾN TRÚC MEDIASTORE & RIGHT PANEL DRAWER (INDEPENDENT MEDIA VAULT)

### 5.1. Bằng Chứng Thực Nghiệm & Đặc Tả Hợp Đồng Mạng Zalo PC
Bằng chứng mã nguồn tại `compact-app-pc.js:15052218` xác lập cấu trúc chuẩn của API MediaStore:

```
[Client ctdozlite]
       │
       │  POST {media_store_domain}/api/mediastore/preview?{common_params}&params={AES_ENCRYPTED}
       │  (Body: NULL / EMPTY)
       ▼
[Zalo Media Vault Server]
       │
       ├─► Giải mã Query params: {"group_id": "...", "imei": "...", "limit": 50}
       │   (Với send2me: chỉ có {"limit": 50})
       ▼
[Response JSON: error_code: 0, data: { items: [...], has_more: true/false, last_id: "..." }]
```

1. **Endpoint và Phương Thức:**
   - **Nhóm & Trò chuyện 1-1:**
     - URL: `{media_store}/api/mediastore/preview?{common_params}&params={cipher}`
     - Method: `POST` (với body rỗng `null`)
     - Resolver domain: `media_store` (trong `src/service_resolver.rs`)
     - Telemetry CmdId: `11843`
     - Payload gốc trước khi mã hóa AES:
       ```json
       {
         "group_id": "123456789",
         "imei": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
         "limit": 50
       }
       ```
   - **Cloud Của Tôi (Send2me / zCloud):**
     - URL: `{media_store_send2me}/api/media/oneone/preview?{common_params}&params={cipher}`
     - Method: `POST` (với body rỗng `null`)
     - Resolver domain: `media_store_send2me`
     - Telemetry CmdId: `12221`
     - Payload gốc trước khi mã hóa AES (**tuyệt đối không có group_id và imei**):
       ```json
       {
         "limit": 50
       }
       ```
2. **Endpoint Phân Trang Danh Sách Chi Tiết (Pagination List):**
   - URL: `{media_store}/api/mediastore/list?{common_params}&params={cipher}` (Direct/Group) hoặc `/api/media/oneone/list` (Send2me).
   - Method: `GET`
   - Telemetry CmdId: `11841` (Group) / `12220` (Send2me).
   - Payload:
     ```json
     {
       "media_type": "media", // "media" (ảnh/video), "file", "link"
       "group_id": "123456789",
       "limit": 30,
       "last_id": "987654321" // Con trỏ phân trang (tuỳ chọn)
     }
     ```

### 5.2. Cấu Trúc Bộ Nhớ Đệm Media Độc Lập Cho Right Panel
Tách biệt hoàn toàn việc lưu trữ media của Right Drawer ra khỏi mảng tin nhắn chat `thread.messages`. Trong state GUI (`src/gui.rs` hoặc `src/gui/workspace.rs`), định nghĩa cấu trúc lưu trữ độc lập theo tài khoản, hội thoại và từng thể loại media:

```rust
#[derive(Debug, Clone, Default)]
pub struct CategoryMediaVault {
    pub items: Vec<crate::server_media::ServerMediaItem>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub is_loading: bool,
    pub is_initial_loaded: bool,
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ConversationMediaVault {
    pub photos: CategoryMediaVault,
    pub files: CategoryMediaVault,
    pub links: CategoryMediaVault,
}

// Lưu trữ trong App State theo khoá: (account_key, conversation_id)
pub type MediaVaultRegistry = std::collections::HashMap<(String, String), ConversationMediaVault>;
```

### 5.3. Quy Trình Nạp Dữ Liệu Cho Right Drawer
1. Khi người dùng bấm mở Right Drawer (`self.info_drawer_open = true`):
   - Kiểm tra `vault` của cuộc trò chuyện hiện tại.
   - Nếu `!vault.photos.is_initial_loaded`: phát lệnh `WorkerCmd::FetchServerMedia` nạp trang đầu tiên cho Hình ảnh (`ServerMediaType::Photo`).
   - Tương tự khi người dùng chuyển sang tab "Tệp tin" hoặc "Liên kết", chỉ nạp theo yêu cầu (On-Demand lazy loading) nếu tab đó chưa từng được tải.
2. Trải nghiệm người dùng:
   - Hiển thị spinner skeleton nhẹ tại panel trong lúc tải.
   - Khi nhận dữ liệu: render lưới ảnh/danh sách tệp tin ngay lập tức. Kể cả tài khoản vừa đăng nhập mới tinh và chưa hề sync điện thoại, 50 file/ảnh gần nhất trên server vẫn hiển thị đầy đủ!
   - Khi cuộn xuống cuối danh sách trong drawer: tự động nạp trang tiếp theo qua con trỏ `next_cursor`.

---

## 6. CƠ CHẾ LAZYLOAD / INFINITE SCROLL & NEO VỊ TRÍ CUỘN (SCROLL ANCHORING)

### 6.1. Nguyên Lý Đọc Phân Tầng: SQLite Trước, Server Sau
Để tạo cảm giác mượt mà và tối ưu hóa tài nguyên mạng, timeline tin nhắn áp dụng nguyên lý phân tầng:
1. **Tầng 1 (Local SQLite Archive):** Khi cuộn lên đỉnh, ưu tiên truy vấn SQLite cục bộ (`store.hydrate_selected_threads` với `before_msg_id`). Tốc độ phản hồi cục bộ cực nhanh (< 10ms), không tiêu tốn băng thông.
2. **Tầng 2 (Server History Fallback):** Khi SQLite cục bộ đã hết tin nhắn (`local_exhausted == true`) nhưng cờ `server_has_more` vẫn bật: ứng dụng mới gửi request lấy lịch sử từ server.
3. **Tầng 3 (Phone Sync Boundary):** Khi máy chủ báo không còn tin nhắn cũ hơn, hiển thị banner thông báo chuẩn Zalo: *"Bạn muốn xem tin nhắn cũ hơn? Vui lòng thực hiện Đồng bộ dữ liệu từ điện thoại."*

### 6.2. Thuật Toán Neo Vị Trí Cuộn (Scroll Anchoring Algorithm)
Thách thức lớn nhất của Infinite Scroll trong `egui::ScrollArea` là khi các phần tử mới được chèn vào đầu mảng (`messages.insert(0, ...)`), tổng chiều cao nội dung tăng lên, khiến tầm mắt người dùng bị nhảy vọt lên đầu hoặc giật nảy màn hình.

Thuật toán neo vị trí cuộn được thiết kế như sau:
```rust
pub struct TimelineScrollAnchorState {
    /// Định danh duy nhất của tin nhắn đang hiển thị ở đỉnh viewport trước khi nạp thêm
    pub anchor_message_id: Option<String>,
    /// Khoảng cách từ đỉnh tin nhắn neo đến đỉnh viewport (pixel offset)
    pub anchor_relative_y: f32,
    /// Cờ đánh dấu đang trong chu kỳ phục hồi vị trí cuộn
    pub pending_restore: bool,
}
```
1. **Trước khi kích hoạt tải thêm (Trigger Phase):**
   - Lắng nghe sự kiện cuộn: Khi `scroll_offset.y < 80.0_f32` (cách đỉnh dưới 80px) và `!thread.is_loading_more && thread.has_more_history`:
   - Xác định tin nhắn đầu tiên đang nhìn thấy trong viewport -> lưu `anchor_message_id = Some(first_visible_msg.id)`.
   - Ghi lại vị trí tương đối của tin nhắn đó.
   - Bật cờ `thread.is_loading_more = true`.
   - Hiển thị spinner nhẹ ở đỉnh timeline: `ui.spinner()`.
2. **Sau khi nhận dữ liệu mới (Prepend Phase):**
   - Chèn mảng tin nhắn cũ vào đầu `thread.messages`. Khử trùng lặp qua `msg_id`.
   - Bật cờ `anchor_state.pending_restore = true`.
3. **Phục hồi trong khung hình tiếp theo (Render & Restore Phase):**
   - Trong lượt render kế tiếp, duyệt tìm vị trí của `anchor_message_id`.
   - Sử dụng `ui.scroll_to_rect(anchor_rect, Some(egui::Align::TOP))` kết hợp bù trừ `anchor_relative_y`.
   - Người dùng cảm thấy các tin nhắn cũ xuất hiện phía trên một cách tự nhiên, vị trí mắt đọc hoàn toàn tĩnh lặng, không hề bị rung giật.

---

## 7. MÀN HÌNH CHÀO MỪNG (WELCOME ONBOARDING) & GIAO DIỆN MẶC ĐỊNH SÁNG (LIGHT THEME)

### 7.1. Chuyển Đổi Mô Hình Quản Lý Lựa Chọn Hội Thoại
Hiện tại, `self.selected_thread_index: usize` bị ép buộc phải trỏ vào một số nguyên hợp lệ từ 0 đến `threads.len() - 1`. Do đó, ứng dụng luôn bị rơi vào tình trạng "phải chọn một hội thoại", dẫn tới việc tự động nhảy vào "Cloud của tôi" khi mở app.

- **Giải pháp chuyển đổi an toàn (Zero Breaking Changes):**
  - Bổ sung trường quản lý trạng thái:
    ```rust
    pub active_thread_id: Option<String>,
    ```
  - Khi khởi động vào `Workspace`: `active_thread_id` được gán là `None`.
  - Trong logic render chính của Workspace (`src/gui/workspace.rs`):
    ```rust
    match &self.active_thread_id {
        Some(tid) if self.threads.iter().any(|t| &t.id == tid) => {
            // Render khung chat bình thường
            self.render_active_conversation_view(ui, tid);
        }
        _ => {
            // Render màn hình Welcome Onboarding chuẩn mực Zalo PC
            self.render_welcome_onboarding_view(ui);
        }
    }
    ```
  - Khi người dùng click vào bất kỳ item nào trong danh sách chat bên trái: gán `self.active_thread_id = Some(clicked_thread.id.clone())`. Màn hình chào mừng tự động đóng lại.

### 7.2. Đặc Tả Giao Diện Màn Hình Welcome Onboarding
Dựa trên bằng chứng component `chat-onboard` (module `/KU8`, class `P` của Zalo PC):
- **Bố cục:** Căn giữa toàn bộ vùng làm việc bên phải.
- **Tiêu đề chính:** *"Chào mừng bạn đến với Zalo PC!"* (Font Segoe UI / Roboto, kích thước 22pt, đậm, màu sắc tương phản cao).
- **Phụ đề:** *"Khám phá những tiện ích hỗ trợ làm việc và trò chuyện cùng người thân, bạn bè được tối ưu hoá cho máy tính của bạn."* (Font 13pt, màu chữ phụ dịu nhẹ).
- **Carousel Slides Giới Thiệu Tính Năng Đã Xác Minh Của Ứng Dụng:**
  - *Slide 1 (Truyền Tệp Lớn):* "Gửi file nhanh chóng, hỗ trợ kéo thả tệp tin và xem trước tài liệu văn phòng trực tiếp."
  - *Slide 2 (Mã Hóa Đầu Cuối):* "Bảo mật hội thoại tối đa với công nghệ Mã hóa đầu cuối (E2EE) chuẩn Signal Protocol."
  - *Slide 3 (Hiệu Suất Windows 11 ARM64):* "Tối ưu hóa native cho Windows 11 ARM64, tiêu thụ bộ nhớ siêu thấp, khởi động tức thì."
  - *Slide 4 (Kho Lưu Trữ Đám Mây):* "Quản lý dữ liệu tập trung với Cloud của tôi và Kho lưu trữ Media server thông minh."
- **Điều khiển:**
  - Nút chuyển slide qua lại (`<` và `>`) mượt mà.
  - Hàng chấm tròn định vị trang (Pagination Dots): chấm hiện tại sáng màu chủ đạo Zalo (`#0068FF`).

### 7.3. Thiết Lập Giao Diện Mặc Định Sáng (Light Theme Parity)
- **Căn cứ thực nghiệm:** Zalo PC và Zalo Web luôn mặc định sử dụng Giao diện Sáng (Light Theme) khi cài đặt mới.
- **Hiện trạng fork:** `src/gui.rs:1166` đang gán mặc định `theme_mode: "dark".into()`.
- **Hợp đồng điều chỉnh:**
  1. Trong `Settings::default()` (`src/gui.rs:1166`):
     ```rust
     theme_mode: "light".into(), // Đổi từ "dark" sang "light"
     ```
  2. Trong hàm xác định giao diện lúc khởi tạo (`src/gui.rs:1831`):
     ```rust
     let dark_mode = match loaded_settings.theme_mode.as_str() {
         "light" => false,
         "dark" => true,
         _ => false, // Fallback mặc định là Light Mode thay vì Dark Mode
     };
     ```
  3. Bảng màu Light Palette giữ nguyên sự tinh tế, độ tương phản cao, tuân thủ nghiêm ngặt tiêu chuẩn `docs/standards/ui-ux-consistency.md`. Người dùng vẫn có toàn quyền chuyển sang Dark Mode hoặc System trong menu Cài đặt.

---

## 8. LỘ TRÌNH TRIỂN KHAI PHÂN KỲ (PHASED IMPLEMENTATION PLAN)

Kế hoạch chia thành 4 giai đoạn độc lập, có thể kiểm chứng riêng biệt và bàn giao theo quy trình audit Codex:

```
┌─────────────────────────────────────────────────────────────────────────┐
│ Giai Đoạn 1: Tối Ưu Hóa Preload, Tách Baseline & Watchdog Bảo Vệ        │
│ ├─ Áp dụng TTL 36h cho danh bạ (đọc SQLite trước, hẹn giờ ngầm)         │
│ ├─ Gửi map con trỏ { id: last_msg_id } cho get_last_msgs                │
│ └─ Watchdog 4s cho baseline fetch; mở Workspace không nghẽn             │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Giai Đoạn 2: Kết Nối MediaStore Server Cho Right Drawer                 │
│ ├─ Tái sử dụng trọn vẹn src/server_media.rs                             │
│ ├─ Tạo MediaVaultRegistry độc lập theo (account, thread, category)       │
│ └─ Kết nối WorkerCmd::FetchServerMedia & GuiEvent::ServerMediaLoaded    │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Giai Đoạn 3: Triển Khai Lazyload Timeline & Thuật Toán Neo Cuộn         │
│ ├─ Phân tầng: Đọc tiếp SQLite trước, chạm đáy mới fetch server          │
│ ├─ Phát hiện cuộn lên đỉnh (scroll_offset.y < 80px) + Spinner loading    │
│ └─ Neo vị trí cuộn qua anchor_message_id, triệt tiêu rung giật viewport │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│ Giai Đoạn 4: Màn Hình Chào Mừng & Thiết Lập Giao Diện Mặc Định Sáng     │
│ ├─ Chuyển đổi an toàn sang active_thread_id: Option<String>             │
│ ├─ Render Welcome Onboarding Carousel khi active_thread_id.is_none()    │
│ └─ Đặt theme_mode mặc định là "light" trong Settings và App State       │
└─────────────────────────────────────────────────────────────────────────┘
```

### Chi Tiết Từng Giai Đoạn:

#### Giai Đoạn 1: Tối Ưu Hóa Preload, Tách Baseline & Watchdog Bảo Vệ
- **Mục tiêu:** Rút ngắn thời gian khởi động, đưa người dùng vào Workspace nhanh nhất có thể mà không bị nghẽn bởi timeout mạng.
- **Nhiệm vụ cụ thể:**
  1. `src/gui/worker.rs`: Trong `preload_startup_data`, tách riêng việc nạp cache SQLite và gọi mạng. Kiểm tra thời gian `last_friend_sync`; nếu < 36 giờ, bỏ qua gọi mạng `/api/social/friend/getfriends` lúc mở app, chỉ nạp bạn bè từ SQLite.
  2. `src/chat.rs`: Bổ sung khả năng truyền con trỏ `{ id: last_msg_id }` cho `chat::get_last_msgs`. Khi cache có tin nhắn, truyền con trỏ để server chỉ trả delta. Khi cache rỗng, tiếp tục truyền `{}`.
  3. Đặt watchdog timeout 4s cho toàn bộ bước nạp mạng ban đầu. Nếu mạng chậm hoặc timeout, phát dữ liệu đã có và chuyển ngay sang trạng thái sẵn sàng (Ready).
- **Kiểm chứng:** Test đơn vị cho logic tính TTL 36h và parser delta preview.

#### Giai Đoạn 2: Kết Nối MediaStore Server Cho Right Drawer
- **Mục tiêu:** Xóa bỏ hoàn toàn hiện tượng Right Drawer bị trống trơn sau khi đăng nhập mới; hiển thị đầy đủ ảnh, file, link của cả hội thoại thường và "Cloud của tôi".
- **Nhiệm vụ cụ thể:**
  1. `src/gui.rs`: Khởi tạo cấu trúc `MediaVaultRegistry` trong App State.
  2. `src/gui.rs`: Nối sự kiện `GuiEvent::ServerMediaLoaded` cập nhật trực tiếp dữ liệu vào `MediaVaultRegistry` tương ứng với `(account_key, thread_id, category)`.
  3. `src/gui/workspace.rs`: Thay thế đoạn mã duyệt `thread.messages` tại dòng 11063 bằng việc đọc từ `MediaVaultRegistry`.
  4. Khi người dùng mở Drawer hoặc đổi tab: phát lệnh `WorkerCmd::FetchServerMedia` nếu category đó chưa có dữ liệu.
- **Kiểm chứng:** Test mock `ServerMediaPage` với 50 ảnh/file, kiểm tra render danh sách và phân trang.

#### Giai Đoạn 3: Triển Khai Lazyload Timeline & Thuật Toán Neo Cuộn
- **Mục tiêu:** Thay thế nút bấm thủ công "Tải thêm tin nhắn cũ" bằng cơ chế cuộn tự động mượt mà.
- **Nhiệm vụ cụ thể:**
  1. `src/gui/worker.rs`: Mở rộng `WorkerCmd::LoadGroupHistory` với tham số `before_msg_id: Option<String>` (hoặc sử dụng `LoadArchiveHistory`).
  2. `src/gui/workspace.rs`: Lắng nghe vị trí `scroll_offset.y` trong timeline. Khi cuộn gần đỉnh: lưu `anchor_message_id`, hiển thị spinner đỉnh và gửi lệnh tải thêm.
  3. Khi nhận mảng tin nhắn cũ: chèn vào đầu `thread.messages`, tính toán lại vị trí cuộn và phục hồi tọa độ hiển thị để không làm nhảy khung hình.
- **Kiểm chứng:** Test logic chèn mảng tin nhắn, khử trùng lặp và tính toán scroll offset bù trừ.

#### Giai Đoạn 4: Màn Hình Chào Mừng & Thiết Lập Giao Diện Mặc Định Sáng
- **Mục tiêu:** Khôi phục trải nghiệm chuẩn Zalo PC khi vừa mở ứng dụng và thiết lập giao diện Sáng mặc định.
- **Nhiệm vụ cụ thể:**
  1. `src/gui.rs`: Thêm `active_thread_id: Option<String> = None`.
  2. `src/gui/workspace.rs`: Nếu `active_thread_id.is_none()`, render component `render_welcome_onboarding_view` với carousel 4 slide giới thiệu tính năng.
  3. Khi click chọn hội thoại trong sidebar: gán `active_thread_id = Some(tid)`.
  4. `src/gui.rs`: Sửa `theme_mode` mặc định thành `"light".into()` trong `Settings::default()` và nhánh fallback.
- **Kiểm chứng:** Kiểm tra khởi động không chọn nhầm Cloud của tôi, hiển thị Welcome Screen đầy đủ và theme Sáng áp dụng chuẩn xác.

---

## 9. MA TRẬN KIỂM CHỨNG, NGHIỆM THU KÉP & CÂU HỎI PHẢN BIỆN (VERIFICATION & CODEX CRITIQUE)

### 9.1. Ma Trận Tiêu Chuẩn Nghiệm Thu Đa Tầng

| Hạng Mục Kiểm Chứng | Cấp Độ 1: CODE (Unit Test / Compile) | Cấp Độ 2: FIXTURE (Mock / Recorded Replay) | Cấp Độ 3: NATIVE / DEVICE (Windows 11 ARM64) |
| :--- | :--- | :--- | :--- |
| **TTL 36h Danh Bạ** | Test logic so sánh timestamp `Date.now() - last_sync`: đúng 36h (`1296e5` ms) mới kích hoạt fetch mạng. | Replay payload SQLite danh bạ mẫu: xác nhận không có outbound HTTP request tới `/getfriends`. | Khởi động app lần 2 trên ARM64: xác nhận vào màn hình chính ngay lập tức, log không ghi nhận network request bạn bè. |
| **Watchdog Preload** | Test timeout promise: khi mock mạng bị delay > 4s, trigger watchdog tự động phát sinh sự kiện chuyển Ready. | Giả lập request `get_last_msgs` trả về lỗi mạng: pipeline không bị treo, chuyển vào Workspace an toàn. | Ngắt mạng ngẫu nhiên lúc đăng nhập: app vẫn mở được dữ liệu offline đã lưu từ trước. |
| **MediaStore Right Drawer** | Test parser `try_parse_server_media_list`: bóc tách chuẩn xác 50 ảnh, 50 file, 50 link từ JSON fixture. | Replay payload MediaStore: kiểm tra `MediaVaultRegistry` cập nhật độc lập, không ghi đè `thread.messages`. | Mở Right Drawer của một nhóm trên thiết bị thật: 50 ảnh/file hiển thị đầy đủ ngay cả khi chưa sync tin nhắn. |
| **Lazyload Timeline** | Test hàm ghép mảng tin nhắn: chèn tin cũ vào đầu, khử trùng lặp qua ID, giữ nguyên thứ tự thời gian. | Giả lập thao tác cuộn đỉnh trong headless harness: kiểm tra cờ `is_loading_more` và lệnh IPC phát ra. | Thao tác cuộn chuột lên đỉnh trên app ARM64: spinner hiện lên, tin cũ tải ra mượt mà, vị trí đọc không bị nhảy giật. |
| **Welcome Screen & Light Theme** | Test khởi tạo App State: `active_thread_id == None` và `settings.theme_mode == "light"`. | Giả lập render Workspace khi `active_thread_id` là None: kiểm tra component Welcome Screen được gọi thay vì Chat View. | Chạy app với profile mới tinh: giao diện nền trắng sáng chuẩn Zalo, cột phải hiện slide chào mừng, không tự nhảy vào Cloud. |

### 9.2. Câu Hỏi Phản Biện Chuyên Sâu Cho Phiên Codex Audit (Critique Focus)
Tài liệu này bàn giao cho AI Orchestrator và Codex Audit tập trung phản biện các góc nhìn sau:
1. **Về An Toàn Con Trỏ Lịch Sử (Cursor Invariant):** Việc truyền map `{ id: last_msg_id }` trong `get_last_msgs` có khả năng làm sót tin nhắn nếu một tin nhắn ở giữa bị rớt do mạng không? Có cần cơ chế reconcile số lượng tin nhắn (`unread_count` / `seq`) hay không?
2. **Về Quản Lý Bộ Nhớ MediaVault:** Khi người dùng mở hàng chục hội thoại và tải kho media của từng hội thoại, `MediaVaultRegistry` có nguy cơ phình to RAM không? Có nên áp dụng chính sách LRU cache giới hạn tối đa 10 hội thoại gần nhất trong bộ nhớ?
3. **Về Độ Trễ Tương Tác Của Scroll Anchor:** Trên các màn hình cảm ứng (Touchscreen) hoặc bàn rê chuột (Precision Touchpad) của thiết bị Surface ARM64, sự kiện cuộn phát sinh liên tục với tần số cao (120Hz). Thuật toán neo vị trí cuộn có cần cơ chế debounce hoặc frame-locking để tránh tính toán sai tọa độ `delta_y` không?
4. **Về Đồng Bộ Trạng Thái Phím Tắt:** Khi `active_thread_id` là `None`, các tổ hợp phím tắt như `Ctrl + F` (tìm kiếm trong chat), `Ctrl + Up/Down` (chuyển hội thoại), hoặc phím `Esc` (đóng drawer) sẽ ứng xử thế nào để không gây panic do unwrapping `None`?

---
*Hồ sơ đặc tả kỹ thuật v2.0 được lưu trữ chính thức tại `C:\Projects\ZaloLite\docs\research\2026-11-25-zalo-native-preload-lazyload-mediastore-parity-spec.md`. Sẵn sàng cho việc rà soát và phản biện độc lập.*
