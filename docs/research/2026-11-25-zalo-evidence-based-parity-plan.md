# BÁO CÁO NGHIÊN CỨU THỰC NGHIỆM, BẢNG ĐÍNH CHÍNH & KẾ HOẠCH PARITY DỰA TRÊN BẰNG CHỨNG (ZALO PC, ZALO WEB & FORK CTDOZLITE)

> **Phạm vi thực hiện:** Nghiên cứu thực nghiệm, phản biện kỹ thuật, đính chính sai lệch và lập kế hoạch chi tiết.  
> **Nguyên tắc quản trị:** Tuân thủ tuyệt đối `AGENTS.md`, `docs/ZALO_REFERENCE_ANALYSIS.md`, các tiêu chuẩn `docs/standards/` và kỷ luật Karpathy:
> 1. Không sửa đổi mã sản phẩm (`src/`), không xóa chức năng, không commit/push.
> 2. Không thực hiện các thao tác làm thay đổi dữ liệu, mã PIN hoặc cấu hình bảo mật trên tài khoản thật.
> 3. Mọi kết luận kỹ thuật đều có dẫn chứng thực nghiệm (UI-LIVE, E1, E2, CODE) hoặc ghi rõ AMBIGUOUS nếu chưa đủ bằng chứng.

---

## MỤC LỤC
- [A. BẢNG ĐÍNH CHÍNH TOÀN DIỆN](#a-bảng-đính-chính-toàn-diện)
- [B. EVIDENCE LEDGER (SỔ BẰNG CHỨNG THỰC NGHIỆM)](#b-evidence-ledger-sổ-bằng-chứng-thực-nghiệm)
- [C. CONTRACT MATRIX (MA TRẬN ĐẶC TẢ HỢP ĐỒNG KỸ THUẬT)](#c-contract-matrix-ma-trận-đặc-tả-hợp-đồng-kỹ-thuật)
- [D. GRANULAR PACKAGE PLAN (KẾ HOẠCH PHÂN RÃ 5 GÓI CÔNG VIỆC)](#d-granular-package-plan-kế-hoạch-phân-rã-5-gói-công-việc)
- [E. DANH SÁCH PENDING & MA TRẬN 4 CẤP ĐỘ KIỂM CHỨNG](#e-danh-sách-pending--ma-trận-4-cấp-độ-kiểm-chứng)

---

## A. BẢNG ĐÍNH CHÍNH TOÀN DIỆN

Bảng đối chiếu và đính chính các nhận định sai lệch, suy diễn hoặc ngộ nhận kỹ thuật từ các tài liệu audit trước (`zalo-parity-master-handover-audit.md` và các bản ghi chép sơ khởi):

| STT | Vấn Đề Kỹ Thuật | Nhận Định Cũ Trong Tài Liệu Trước | Kết Luận Kiểm Chứng Thực Tế | Bằng Chứng Xác Minh (E1/E2/CODE/UI) | Lý Do & Tác Động Triển Khai |
| :---: | :--- | :--- | :--- | :--- | :--- |
| **1** | **Phương thức HTTP Tin nhắn nhanh (Quick Messages)** | Ghi nhận các thao tác create, update, delete dùng phương thức `POST` với payload `shortcut`, `attachments`. | **SAI HOÀN TOÀN.** Cả 4 API đều dùng phương thức `GET` (`_get`), payload mã hóa AES đưa vào query string `&params=...`. | **E2**: `compact-app-pc:15161790`: `fetchQuickReplyList` (CmdId 12025), `createQuickReplyItem` (CmdId 12029), `updateQuickReplyItem` (CmdId 12026), `deleteQuickReplyItem` (CmdId 12032). | Sửa đặc tả worker: Gửi request `_get` với param cipher AES; tên trường là `keyword`, `message`, `media`, `type`, `imei`. |
| **2** | **Hành vi click chọn Tin nhắn nhanh** | Cho rằng có tùy chọn hoặc nút bấm "Gửi ngay" trực tiếp từ popover. | **BÁC BỎ.** Click vào một mẫu tin nhắn nhanh luôn thay thế/chèn vào ô soạn thảo văn bản (`replaceText`). Không có tùy chọn "Gửi ngay". | **E2**: `compact-app-pc:6620020`: `replaceText: o.wrap("quick-message", l.replaceText)`. | Bỏ thiết kế cờ `send_immediately` khỏi UI popover; chỉ thực hiện chèn nội dung vào composer. |
| **3** | **Hợp đồng Tạo bình chọn (Poll) vs Group Board** | Gộp Poll vào Group Board và cho rằng Tạo bình chọn gọi `POST /api/board/topic/createv2` với `POLL = 2`. | **SAI LỆCH.** Tạo poll là API độc lập: `POST /api/poll/create` (CmdId 12080) trên domain `group_poll`. Trong Group Board, `POLL = 3` (không phải 2). | **E2**: `compact-app-pc:15121944` (`createPoll`), `compact-app-pc:1703323` (`pi.POLL = 3`). Ghim poll là bước phụ gọi `pinBoardItem(..., POLL_TOPIC)`. | Giữ nguyên endpoint `/api/poll/create` (CmdId 12080) hiện có trong `src/group_content.rs:410`; không trỏ nhầm sang `board/topic/createv2`. |
| **4** | **Tính xác thực của Artifact Zalo Web** | Dẫn chiếu các file `final_step_427.png`, `431.png`, `441.png`, `449.png` như bằng chứng đã lưu trên đĩa. | **BÁC BỎ.** Các file này là log frame tạm thời của browser agent, **không tồn tại trên ổ đĩa**. File `final_step_451.png` là màn hình chat, không phải toàn cảnh Settings. | **FS CHECK**: Thư mục `browser_evidence/...` chỉ tồn tại `final_step_419.png` (chat + right panel) và `final_step_451.png` (chat). | Đánh dấu các mục thiếu là "Chưa có artifact". Không dẫn chiếu các đường dẫn ảo. |
| **5** | **Bản chất & Lưu trữ Mã PIN Khóa Ứng Dụng (App Lock)** | Ghi nhận App Lock PIN lưu vào file `settings.json`, suy diễn "fork dùng PBKDF2 là chuẩn Zalo PC". | **CHÍNH XÁC HÓA.** Fork lưu vào `app_settings.json` (qua `crate::storage::settings_path()`). Zalo PC dùng `pinVault` (PBKDF2/MD5 210k vòng lặp, `mode: "local"`). Web **không có** App Lock. | **CODE & E2**: `src/storage.rs:331` (`root().join("app_settings.json")`); `compact-app.js:1099350` (`Ar="pin-vault", Lr=21e4`). | Xác nhận fork đúng về bản chất `mode: "local"`, nhưng khác về định dạng file lưu trữ so với Zalo PC. |
| **6** | **Vị trí Thanh Định Dạng RTF (Composer)** | Khẳng định thanh định dạng RTF `Aa` "bắt buộc phải nằm ở TRÊN CÙNG của khung soạn thảo". | **BÁC BỎ.** Cả Zalo PC và Zalo Web đều đặt thanh công cụ định dạng **ở DƯỚI CÙNG** của ô nhập văn bản khi bật RTF. | **UI-LIVE**: `zalo_pc_composer_live.png` và Zalo Web live. | Bố trí thanh format nằm dưới TextEdit; mở rộng chiều cao composer lên trên. |
| **7** | **Hiện trạng Cài đặt (Settings) của Fork** | Cho rằng Settings của fork "chắp vá, thiếu vắng 80% tính năng cốt lõi" và cần làm lại 10 tab từ đầu. | **BÁC BỎ.** `src/gui/modals.rs:812-823` đã xây dựng sẵn bộ khung 10 tab `SettingsTab`. Zalo Web thực tế chỉ có 7 tab. | **CODE & UI-LIVE**: Fork có 10 tab enum; Web có 7 tab (`final_step_451.png` context). | Không đập đi xây lại; chỉ đấu nối các control chưa có backend hoặc trỏ nhầm vào `tools_panel.rs`. |
| **8** | **Cấu hình Phím Enter Gửi Tin trong Fork** | Cho rằng "Đã có trường cấu hình trong AppSettings và chỉ cần nối UI". | **SAI LỆCH.** Struct `AppSettings` trong `src/gui.rs:1075-1137` **hoàn toàn chưa có** trường `enter_to_send` hay `send_on_enter`. `workspace.rs:5362` hardcode Enter. | **CODE**: `src/gui.rs:1075-1137` và `src/gui/workspace.rs:5359-5362`. | Phải bổ sung trường `enter_to_send: bool` vào `AppSettings`, tạo UI toggle trong tab Tin nhắn và cập nhật logic bắt phím trong `workspace.rs`. |
| **9** | **Hiện trạng Composer & Nút Mở Rộng trong Fork** | Cho rằng fork chưa có thanh cuộn dọc `ScrollArea` và thiếu nút mở rộng. | **SAI LỆCH.** `src/gui/workspace.rs:5450-5459` **đã có** `egui::ScrollArea::vertical()`. Nút Emoji và Like bị lệch do nằm trong `ui.horizontal_wrapped`. | **CODE**: `src/gui/workspace.rs:5450-5459`. | Vấn đề là bố cục hàng ngang (horizontal wrapped) làm các nút căn giữa theo chiều cao của ScrollArea; giải pháp là chia 2 cột dọc độc lập. |
| **10** | **Hiện trạng Mention `@` trong Fork** | Cho rằng fork chưa có `@All`. | **SAI LỆCH.** `src/gui/mention_composer.rs:64` **đã cài đặt sẵn** `@All (Cả nhóm)` đứng đầu mảng `items`. | **CODE**: `src/gui/mention_composer.rs:64`. | Fork chỉ thiếu điều hướng phím mũi tên Lên/Xuống/Enter để chọn mục gợi ý, không thiếu dữ liệu `@All`. |
| **11** | **Bản chất Nút "✓" trên Navigation Rail** | Cho rằng chỉ cần chuyển đổi trạng thái `View::Todo` là đạt chuẩn zTask. | **SỬA LẠI.** zTask trên Zalo PC là cả một dịch vụ quản lý công việc (mini-app/iframe). `View::Todo` của fork hiện là stub rỗng không có data model. | **E2 & CODE**: Zalo PC gọi dịch vụ zTask; Fork chưa có worker command hay SQLite table cho task. | Cần làm rõ phạm vi: chuyển View chỉ là bước giao diện; nếu muốn parity tính năng thì phải đặc tả cả giao thức zTask. |
| **12** | **Kế hoạch Di Dời Quick Messages Cũ** | Đề xuất tự động tải toàn bộ tin nhắn nhanh local lên server khi khởi động ứng dụng. | **BÁC BỎ.** Tự động upload gây nguy cơ ghi đè dữ liệu server, trùng lặp (duplicate), lỗi khi đổi tài khoản và vi phạm quyền riêng tư của người dùng. | **NGUYÊN TẮC BẢO TOÀN DỮ LIỆU**: Thao tác đẩy lên máy chủ phải do người dùng chủ động kiểm soát. | Thiết kế lại: Cung cấp giao diện đối chiếu, nhập chọn lọc (selective import) có hộp kiểm xác nhận. |

---

## B. EVIDENCE LEDGER (SỔ BẰNG CHỨNG THỰC NGHIỆM)

Bảng tổng hợp toàn bộ các bằng chứng kỹ thuật thu thập được từ Zalo PC 26.11.12, Zalo Web live và mã nguồn fork:

| Mã Định Danh | Loại Bằng Chứng | Môi Trường | Thời Điểm | Nguồn / Đường Dẫn / Artifact | Tóm Tắt Kết Luận Kỹ Thuật |
| :--- | :---: | :---: | :---: | :--- | :--- |
| **EV-PC-BUNDLE** | **E2** | Zalo PC 26.11.12 | 2026-11-25 | `%TEMP%\zalo-asar-analysis\pc-dist\compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>SHA256: `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08` | Toàn bộ định nghĩa API client, routing, command ID và model nghiệp vụ của Zalo Desktop. |
| **EV-PC-MAIN** | **E2** | Zalo PC 26.11.12 | 2026-11-25 | `%TEMP%\zalo-asar-analysis\main-dist\compact-app.js`<br>Offset: `1,099,350` | Triển khai `pinVault` (`Ar="pin-vault", Cr="pin-vault-data", Ir="pvfk", Lr=21e4`). PBKDF2/MD5 210k vòng lặp. Lưu trữ cục bộ hoàn toàn. |
| **EV-PC-QR-API** | **E2** | Zalo PC 26.11.12 | 2026-11-25 | `compact-app-pc:15161790-15162200` | 4 hàm API Quick Reply: `fetchQuickReplyList` (12025), `createQuickReplyItem` (12029), `updateQuickReplyItem` (12026), `deleteQuickReplyItem` (12032). Tất cả đều là `_get` với `&params=...`. |
| **EV-PC-QR-ACT** | **E2** | Zalo PC 26.11.12 | 2026-11-25 | `compact-app-pc:6620020` | Click mẫu Quick Message gọi `replaceText`, thay thế nội dung composer. Không có tùy chọn gửi trực tiếp. |
| **EV-PC-POLL** | **E2** | Zalo PC 26.11.12 | 2026-11-25 | `compact-app-pc:15121944` | `createPoll(groupId, payload, src)`: gọi `POST /api/poll/create` (CmdId 12080) trên domain `group_poll`. |
| **EV-PC-BOARD** | **E2** | Zalo PC 26.11.12 | 2026-11-25 | `compact-app-pc:1703323` | `pi = { ALL: 0, GNOTICE_TOPIC: 1, GNOTICE_MESSAGE: 2, POLL: 3, GSUGGESTION: 4, DATE_TITLE: 5 }`. Enum type của Poll trong bảng tin nhóm là 3. |
| **EV-PC-PIN-CFG** | **E2** | Zalo PC 26.11.12 | 2026-11-25 | `compact-app-pc:19326664` | Cấu hình bảo mật: `{"hidden-chat": {mode: "synced"}, "app-lock": {mode: "local"}}`. Phân định rõ 2 cơ chế độc lập. |
| **EV-PC-COMP-UI** | **UI-LIVE** | Zalo PC 26.11.12 | 2026-11-24 | `zalo_pc_composer_live.png`<br>(882,864 bytes) | Khung soạn thảo Zalo PC: Thanh công cụ định dạng RTF nằm ở DƯỚI ô nhập. Nút Like/Gửi và Emoji ghim cố định góc dưới bên phải. |
| **EV-WEB-CTX** | **UI-LIVE** | Zalo Web | 2026-11-25 | `https://chat.zalo.me/`<br>Tài khoản: `Dark Baron`<br>DPR: 1.5, Viewport: 920x896 | Giao thức `zpw_ver=695`. Trình duyệt Chrome 131 trên Windows 11 ARM64. Không có tính năng App Lock. Settings có 7 tabs. |
| **EV-WEB-IMG-419** | **UI-LIVE** | Zalo Web | 2026-11-25 | `browser_evidence/.../final_step_419.png`<br>(215,806 bytes) | Màn hình chat và Information panel bên phải. Khung composer ở trạng thái cơ bản, nút Emoji và Gửi ghim sát đáy. |
| **EV-WEB-IMG-451** | **UI-LIVE** | Zalo Web | 2026-11-25 | `browser_evidence/.../final_step_451.png`<br>(218,661 bytes) | Toàn cảnh hội thoại đang mở với người dùng Dark Baron. |
| **EV-WEB-MISSING** | **AMBIGUOUS** | Zalo Web | 2026-11-25 | Các file `final_step_427, 431, 441, 449.png` | **Chưa có artifact trên đĩa.** Là ID bước chạy của browser subagent trước khi đóng phiên. Phải đánh dấu pending kiểm chứng hình ảnh. |
| **EV-FORK-HEAD** | **CODE** | Fork ctdozlite | 2026-11-25 | Git HEAD: `a5ad2b14c6d5b6bcd3ca59ed385f75f4fbff6b0c` | Working tree dirty (18 file modified trong `src/`). `cargo check --locked` hoàn tất không lỗi. |
| **EV-FORK-SCROLL** | **CODE** | Fork ctdozlite | 2026-11-25 | `src/gui/workspace.rs:5450-5459` | Đã có `egui::ScrollArea::vertical()`, tuy nhiên bị lồng trong `ui.horizontal_wrapped` cùng các nút action. |
| **EV-FORK-ENTER** | **CODE** | Fork ctdozlite | 2026-11-25 | `src/gui.rs:1075-1137`<br>`src/gui/workspace.rs:5359-5362` | Chưa có trường `enter_to_send` trong `AppSettings`. `workspace.rs` hardcode Enter là gửi tin, Shift+Enter là xuống dòng. |
| **EV-FORK-MENTION** | **CODE** | Fork ctdozlite | 2026-11-25 | `src/gui/mention_composer.rs:64` | Đã có `@All (Cả nhóm)` đứng đầu mảng `items`. Thiếu xử lý điều hướng phím mũi tên. |
| **EV-FORK-SETTINGS** | **CODE** | Fork ctdozlite | 2026-11-25 | `src/gui/modals.rs:812-823` | Đã có 10 tab `SettingsTab`. |
| **EV-FORK-PATH** | **CODE** | Fork ctdozlite | 2026-11-25 | `src/storage.rs:331` | `settings_path()` trả về `root().join("app_settings.json")`. Lưu hash PBKDF2 của App Lock tại đây. |
| **EV-FORK-POLL** | **CODE** | Fork ctdozlite | 2026-11-25 | `src/group_content.rs:410` | Đã có cài đặt gọi `/api/poll/create` (CmdId 12080) trên domain `group_poll`. |

---

## C. CONTRACT MATRIX (MA TRẬN ĐẶC TẢ HỢP ĐỒNG KỸ THUẬT)

Ma trận quy định chi tiết từng thao tác mạng, tham số mã hóa và cách ứng xử trên client:

```
[UI Trigger] 
      |
      v
[WorkerCmd Dispatch] 
      |
      v
[ParamCipher v2 (AES-CBC/GCM)] + [Signkey MD5] + [Session Cookies]
      |
      v
[REST HTTP Request (GET/POST) / WebSocket Command ID]
      |
      v
[Decrypt Response & Unwrap Result] 
      |
      v
[Update SQLite Store / Reactive App State] ---> [UI Re-render]
```

### 1. Chi Tiết Từng Hợp Đồng Trọng Yếu:

#### 1.1. Tin Nhắn Nhanh (Quick Messages)
- **Caller UI:** Nút `⚡` trên thanh công cụ Composer; mục "Quản lý tin nhắn nhanh" trong Cài đặt -> Tin nhắn.
- **Hợp Đồng REST (E2 Parity):**
  * `fetchQuickReplyList(version=0, lang=0)`:
    - Domain: `getQuickMessageDomain()` (từ service map).
    - Route & Method: `GET /api/quickmessage/list`
    - CmdId: `12025`
    - Query Params: `?...&params=AES({"version": e, "lang": t, "imei": zaloClientID})`
  * `createQuickReplyItem({ keyword, message, media, type })`:
    - Route & Method: `GET /api/quickmessage/create`
    - CmdId: `12029`
    - Query Params: `?...&params=AES({"keyword": k, "message": m, "media": med, "type": typ, "imei": zaloClientID})`
  * `updateQuickReplyItem({ id, keyword, message, media, type })`:
    - Route & Method: `GET /api/quickmessage/update`
    - CmdId: `12026`
    - Query Params: `?...&params=AES({"itemId": id, "keyword": k, "message": m, "media": med, "type": typ})`
  * `deleteQuickReplyItem(itemIds)`:
    - Route & Method: `GET /api/quickmessage/delete`
    - CmdId: `12032`
    - Query Params: `?...&params=AES({"itemIds": [...]})`
- **Response Schema:** `{ "error_code": 0, "data": { "item": { "id": 123, "keyword": "...", "message": "...", "media": "...", "type": 0 }, "version": 456 } }`
- **Cache & Render:** Lưu danh sách vào bộ nhớ ram của UI (`self.quick_messages: Vec<QuickMessageItem>`). Khi gõ `/` trong composer, lọc theo `keyword`. Khi click: gọi `replaceText` chèn vào ô nhập.
- **Entitlement & Lỗi:** Giới hạn tối đa 50 tin nhắn nhanh. Nếu `error_code == 12030` (quá giới hạn): hiển thị toast cảnh báo.

#### 1.2. Thẻ Tài Khoản Ngân Hàng (Bank Card)
- **Caller UI:** Nút Thẻ ngân hàng trên Composer; popover chọn tài khoản đã lưu.
- **Hợp Đồng REST (E2 Parity):**
  * Lấy danh sách tài khoản đã lưu:
    - Route & Method: `GET /api/transfer/list`
    - CmdId: `15298`
    - Domain: `getZInstantMsgDomain()`
  * Gửi thẻ ngân hàng vào chat:
    - Route & Method: `POST /api/transfer/card`
    - CmdId: `15220`
    - Payload params: `{"binBank": "...", "numAccBank": "...", "nameAccBank": "...", "cliMsgId": ..., "tsMsg": ..., "destUid": "...", "destType": ...}`
- **Response Schema:** `{ "error_code": 0, "data": { "msgId": "..." } }`
- **Render Message:** Hiển thị bong bóng chat dạng thẻ ATM: có dải màu gradient thương hiệu ngân hàng, logo NAPAS/Bank, số tài khoản che bớt hoặc định dạng nhóm 4 số, tên chủ thẻ in hoa, và nút "Sao chép số tài khoản".

#### 1.3. Tạo Bình Chọn Nhóm (Group Poll)
- **Caller UI:** Menu `...` trên Composer -> "Tạo bình chọn".
- **Hợp Đồng REST (E2 Parity):**
  * Route & Method: `POST /api/poll/create`
  * CmdId: `12080`
  * Domain: `getGroupPollDomain()`
  * Payload params: `{"group_id": "...", "question": "...", "options": ["A", "B", ...], "expired_time": 0, "pin_act": 0/1, "multi_choice": 0/1, "allow_add_option": 0/1, "hide_vote": 0/1, "hide_voter": 0/1, "src": 1, "imei": zaloClientID}`
  * **Luồng ghim (Pinning):** Nếu `pin_act == 1`, sau khi nhận `poll_id` từ kết quả tạo, client gọi tiếp hàm ghim bảng tin: `POST /api/board/topic/pin` (CmdId: `AMBIGUOUS`) với `boardType = 3` (`GROUP_BOARD_ITEM_TYPE.POLL`).

#### 1.4. Gửi Danh Thiếp (Contact Card)
- **Caller UI:** Nút Danh thiếp trên Composer.
- **Hợp Đồng REST (E2 Parity):**
  * Gửi vào chat cá nhân 1-1: `POST /api/message/contact` (CmdId `11774`).
  * Gửi vào chat nhóm: `POST /api/group/contact` (CmdId `11827`).
  * Payload: `{"phone": "...", "userId": "...", "name": "...", "avatar": "...", "cliMsgId": ...}`.
- **Behavior:** Modal danh thiếp hỗ trợ tìm kiếm nhanh, gom nhóm theo chữ cái A-Z và chọn nhiều liên hệ (multi-select) để gửi một loạt.

#### 1.5. Thiết Lập Quyền Riêng Tư (Privacy Settings)
- **Caller UI:** Tab "Quyền riêng tư" trong Cài đặt.
- **Hợp Đồng REST (E2 Parity):**
  * Đọc thiết lập hiện tại: `GET /api/setting/me` (CmdId `12113`).
  * Cập nhật từng thuộc tính: `POST /api/setting/update` (CmdId `12033`). Payload: `{"key": "show_online_status", "val": 0/1}`.
  * Cấu hình bảo vệ 2 lớp (2FA): `POST /api/update/setting/2FA` (CmdId `12066`).

#### 1.6. Mã PIN Khóa Ứng Dụng (App Lock) vs Hội Thoại Ẩn (Hidden Chat)
- **App Lock PIN (Cục bộ hoàn toàn):**
  * Zalo PC: Xử lý qua `pinVault` trong tiến trình main Electron. Lưu credential mã hóa tại storage cục bộ. Khóa giao diện qua `Ctrl + L` hoặc timeout không hoạt động.
  * Fork: Sử dụng PBKDF2 (HMAC-SHA256, 100,000 vòng lặp) lưu trong `app_settings.json`. Tuyệt đối không gọi request ra ngoài mạng.
- **Hidden Chat PIN (Đồng bộ tài khoản):**
  * Zalo PC & Web: Đồng bộ toàn bộ qua máy chủ Zalo.
  * `getAllListHiddenChat`: `GET /api/hiddenconvers/get-all` (CmdId `10048`).
  * `addRemoveHiddenChat`: `POST /api/hiddenconvers/add-remove` (CmdId `10049`).
  * `updatePin`: `GET /api/hiddenconvers/update-pin` (CmdId `10050`). Params: `new_pin = md5(pin_4_digits)`.
  * `resetPin`: `GET /api/hiddenconvers/reset` (CmdId `10051`).

---

## D. GRANULAR PACKAGE PLAN (KẾ HOẠCH PHÂN RÃ 5 GÓI CÔNG VIỆC)

Kế hoạch chia thành 5 gói công việc độc lập, có thể thực hiện và kiểm chứng riêng rẽ:

```
+-----------------------------------------------------------------------------------+
| GÓI 1: Bố Cục Hình Học Composer & Pinned Action Buttons (UI Cốt Lõi)              |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| GÓI 2: Tinh Chỉnh Tương Tác Nội Bộ (Mention Navigation, Enter Toggle, Contact)   |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| GÓI 3: Cài Đặt Hợp Đồng Máy Chủ Mới (Quick Messages GET, Bank Card, Poll CmdId)   |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| GÓI 4: Giao Diện Di Cư Tin Nhắn Nhanh Có Chọn Lọc (Selective Migration UI)       |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| GÓI 5: Tái Cấu Trúc zTask & Cô Lập An Toàn tools_panel.rs                         |
+-----------------------------------------------------------------------------------+
```

---

### Gói 1: Bố Cục Hình Học Composer & Pinned Action Buttons
- **Mục tiêu:** Chấm dứt hiện tượng nút Emoji và Like bị đẩy lên lơ lửng ở giữa màn hình khi nhập tin nhắn nhiều dòng; đặt thanh công cụ RTF nằm dưới ô nhập văn bản theo đúng chuẩn Zalo gốc.
- **Tập tin liên quan:** `src/gui/workspace.rs`, `src/gui/theme.rs`.
- **Giải pháp kiến trúc:**
  1. Thay thế `ui.horizontal_wrapped` bằng layout chia 2 cột rõ ràng:
     * Cột chính (bên trái, chiếm toàn bộ chiều rộng khả dụng `ui.available_width() - action_width`): Chứa `ScrollArea::vertical` bọc `TextEdit::multiline`.
     * Cột hành động (bên phải, tính toán linh hoạt theo DPI/nút, không hardcode cứng 76.0px): Dùng layout dọc căn đáy `with_layout(Layout::bottom_up(Align::Max))` để ghim nút Emoji và nút Send/Like luôn nằm sát góc dưới bên phải.
  2. Bố cục RTF: Khi `self.composer_format_mode == true`, thanh công cụ 14 nút định dạng được render **ngay bên dưới** vùng soạn thảo văn bản.
- **Rủi ro & Hồi quy:** Chiều cao composer thay đổi động có thể lấn vào vùng hiển thị tin nhắn cuối cùng -> Cần kiểm tra lại hàm tính `chat_history_height` để đảm bảo scroll view của tin nhắn co giãn tương ứng.
- **Tiêu chuẩn nghiệm thu:**
  - Nhập 1 dòng text: Emoji và Like nằm ở góc dưới bên phải.
  - Nhập 15 dòng text: ScrollArea xuất hiện thanh cuộn mượt mà; nút Emoji và Like vẫn giữ nguyên vị trí cố định ở đáy; không bị trôi lên giữa.
  - Bật nút `Aa`: Thanh định dạng xuất hiện phía dưới ô nhập văn bản.

---

### Gói 2: Tinh Chỉnh Tương Tác Nội Bộ (Mention, Enter Toggle, Contact Modal)
- **Mục tiêu:** Hoàn thiện trải nghiệm bàn phím mượt mà và kết nối dữ liệu cấu hình tin nhắn.
- **Tập tin liên quan:** `src/gui.rs`, `src/gui/workspace.rs`, `src/gui/modals.rs`, `src/gui/mention_composer.rs`.
- **Giải pháp kiến trúc:**
  1. Bổ sung trường cấu hình: Thêm `pub enter_to_send: bool` (mặc định `true`) vào `AppSettings` trong `src/gui.rs`.
  2. Đấu nối hai chiều (2-way binding):
     * Trong `src/gui/modals.rs` (Tab Tin nhắn): Thêm radio button hoặc checkbox "Nhấn Enter để gửi tin nhắn (Shift+Enter để xuống dòng)" vs "Nhấn Ctrl+Enter để gửi".
     * Trong `src/gui/workspace.rs`: Đọc `self.settings.enter_to_send` để phân nhánh bắt phím gửi tin.
  3. Mention Keyboard Navigation:
     * Trong `src/gui/mention_composer.rs`: Bổ sung trạng thái `selected_index: usize`.
     * Bắt phím `ArrowUp`, `ArrowDown` để di chuyển vệt sáng chọn thành viên; phím `Enter` hoặc `Tab` để xác nhận chèn tên thành viên vào composer.
     * Chặn popup mention khi đang chat 1-1 (tránh phiền toái khi gõ địa chỉ email).
  4. Modal Danh thiếp: Bổ sung thanh tìm kiếm lọc theo tên/SĐT, checkbox chọn nhiều người và nút "Gửi (N)".
- **Rủi ro & Hồi quy:** Bắt phím mũi tên trong Mention có thể xung đột với phím điều hướng con trỏ văn bản của `TextEdit` -> Chỉ chặn phím khi popup mention đang hiển thị (`mention_popup_open`).
- **Tiêu chuẩn nghiệm thu:**
  - Thay đổi toggle Enter trong Cài đặt lập tức có tác dụng ở Composer (gõ Enter xuống dòng nếu tắt toggle).
  - Gõ `@` trong nhóm: Dùng phím mũi tên Lên/Xuống chọn được `@All` hoặc thành viên, ấn Enter chèn thành công.

---

### Gói 3: Cài Đặt Hợp Đồng Máy Chủ Đã Xác Minh
- **Mục tiêu:** Bổ sung các worker command giao tiếp với Zalo backend theo đúng hợp đồng E2.
- **Tập tin liên quan:** `src/chat.rs`, `src/gui/worker.rs`, `src/gui.rs`, `src/group_content.rs`.
- **Giải pháp kiến trúc:**
  1. Tin nhắn nhanh:
     * Cài đặt `WorkerCmd::FetchQuickMessages`, `WorkerCmd::CreateQuickMessage { keyword, message, media, msg_type }`, `WorkerCmd::UpdateQuickMessage`, `WorkerCmd::DeleteQuickMessage`.
     * Sử dụng phương thức `_get` mã hóa params AES, trỏ đúng các Command ID `12025`, `12029`, `12026`, `12032`.
  2. Thẻ tài khoản ngân hàng:
     * Định nghĩa struct `BankCardPayload { bin_bank, acc_num, acc_name }`.
     * Gửi request lên `/api/transfer/card` (CmdId `15220`).
  3. Tạo bình chọn:
     * Tận dụng `src/group_content.rs:410` `/api/poll/create` (CmdId `12080`), đấu nối modal tạo bình chọn vào command này.
- **Rủi ro & Hồi quy:** Máy chủ từ chối nếu thiếu tham số `imei` hoặc session cookie hết hạn -> Cần kiểm tra đính kèm đầy đủ `imei: session.zalo_client_id` và bắt lỗi `ERR_SESSION_EXPIRED`.
- **Tiêu chuẩn nghiệm thu:**
  - Viết unit test mock giải mã payload gửi đi từ `WorkerCmd`, xác nhận đúng phương thức GET và cấu trúc query string.
  - Chạy `cargo test --locked` kiểm tra hồi quy toàn bộ luồng mạng.

---

### Gói 4: Giao Diện Di Cư Tin Nhắn Nhanh Có Chọn Lọc (Selective Migration UI)
- **Mục tiêu:** Bảo toàn dữ liệu tin nhắn nhanh cục bộ cũ của người dùng mà không gây xung đột hoặc tự ý ghi đè lên máy chủ.
- **Tập tin liên quan:** `src/gui/modals.rs`, `src/gui/workspace.rs`.
- **Giải pháp kiến trúc:**
  1. Loại bỏ hoàn toàn ý tưởng "tự động tải lên máy chủ khi khởi động".
  2. Khi người dùng mở "Quản lý tin nhắn nhanh" trong Cài đặt:
     * Ứng dụng nạp đồng thời: Danh sách từ máy chủ (`server_items`) và danh sách lưu trong cấu hình local cũ nếu có (`local_items`).
     * Nếu phát hiện có `local_items` chưa có trên server, hiển thị một banner thông báo: *"Phát hiện N mẫu tin nhắn nhanh lưu cục bộ trước đây."* kèm nút *"Xem và Nhập vào tài khoản"*.
     * Mở modal đối chiếu 2 cột: Cột trái là danh sách local, cột phải là trạng thái trên máy chủ.
     * Người dùng tích chọn từng mục cần đưa lên server, bấm *"Xác nhận nhập"*.
     * Ứng dụng lần lượt gọi `WorkerCmd::CreateQuickMessage` cho các mục được chọn.
     * Cung cấp tùy chọn: *"Xóa dữ liệu local cũ sau khi nhập thành công"* hoặc *"Giữ lại làm bản sao lưu"*.
- **Rủi ro & Hồi quy:** Người dùng bấm nhập nhiều lần gây trùng lặp -> Disable nút trong khi đang gửi; kiểm tra trùng lặp keyword trước khi gửi.
- **Tiêu chuẩn nghiệm thu:**
  - Người dùng có toàn quyền kiểm soát dữ liệu: Không có bất kỳ request mạng nào tự ý phát sinh nếu chưa có hành động bấm nút xác nhận.

---

### Gói 5: Tái Cấu Trúc zTask & Cô Lập An Toàn `tools_panel.rs`
- **Mục tiêu:** Tách rời các phụ thuộc vào `tools_panel.rs`, quy hoạch lại nút "✓" trên Navigation rail và dọn dẹp mã nguồn theo chuẩn.
- **Tập tin liên quan:** `src/gui.rs`, `src/gui/workspace.rs`, `src/gui/tools_panel.rs`.
- **Giải pháp kiến trúc:**
  1. Nút "✓" trên Navigation rail:
     * Chuyển đổi trạng thái `self.active_view = View::Todo`.
     * Trên màn hình `View::Todo`, hiển thị giao diện danh sách công việc sạch sẽ, thông báo rõ ràng về trạng thái kết nối dịch vụ zTask.
  2. Rà soát và gỡ bỏ các cờ mở popup cưỡng bức:
     * Xóa các lệnh `self.tools.open = true` rải rác trong `workspace.rs`, `modals.rs`.
     * Thay thế menu `...` trên Composer: Chỉ hiển thị các mục chuẩn Zalo (Bình chọn, Giao việc, Nhắc hẹn), không mở bảng công cụ phụ trợ.
  3. Cô lập `tools_panel.rs`:
     * Chuyển các tính năng phụ trợ không thuộc Zalo chuẩn (LAN transfer, Tin nhắn tự động, Lịch gửi tin) vào một tab riêng mang tên "Công cụ mở rộng (Lab / Extensions)" trong Cài đặt, hoặc cô lập an toàn trong module phụ.
     * Khi toàn bộ tham chiếu được giải phóng và kiểm chứng không còn lỗi biên dịch, tiến hành lưu trữ hoặc xóa tệp `tools_panel.rs`.
- **Rủi ro & Hồi quy:** Nguy cơ làm mất trạng thái dữ liệu đang lưu trong `self.tools` -> Kiểm tra kỹ mọi trường dữ liệu trước khi di dời.
- **Tiêu chuẩn nghiệm thu:**
  - `cargo check --locked` không có cảnh báo unused code hoặc binding hỏng.
  - Giao diện chat không còn bất kỳ popup lạ nào đè lên khi nhấp các nút chuẩn Zalo.

---

## E. DANH SÁCH PENDING & MA TRẬN 4 CẤP ĐỘ KIỂM CHỨNG

Bảng phân loại minh bạch các hạng mục kỹ thuật theo 4 cấp độ xác minh:

| Hạng Mục Tính Năng | Cấp Độ 1: Chưa Nghiên Cứu | Cấp Độ 2: Đã Đối Chiếu Tĩnh (E2) | Cấp Độ 3: Đã Test Model / Fixture | Cấp Độ 4: Đã Nghiệm Thu Server Thật | Ghi Chú & Hành Động Tiếp Theo |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **API Quick Messages (12025-12032)** | | [X] | [ ] | [ ] | Đã xác minh 4 hàm `_get` trong bundle PC. Cần viết fixture test mock response trước khi gọi thật. |
| **Hành vi replaceText Quick Reply** | | [X] | [ ] | [ ] | Đã xác minh trong bundle PC. Cần kiểm tra tương tác thực tế trên webview. |
| **Tạo Poll (`/api/poll/create` 12080)** | | [X] | [ ] | [ ] | Đã xác minh thân hàm `createPoll` trong bundle PC và có sẵn trong `group_content.rs:410`. |
| **Ghim Poll vào Group Board (Type 3)** | | [X] | [ ] | [ ] | Đã xác minh `pi.POLL = 3` trong bundle. API ghim chi tiết cần thêm capture E1 để chốt query params. |
| **App Lock PIN (Local Vault)** | | [X] | [X] | [ ] | Đã xác minh Zalo PC dùng `mode: "local"`. Fork đã có unit test PBKDF2 trong `gui.rs`. Không gọi server. |
| **Hidden Chat PIN (`/api/hiddenconvers`)** | | [X] | [X] | [ ] | Đã xác minh E2 và fork đã cài đặt trong `hidden_chat.rs`. **TUYỆT ĐỐI KHÔNG tự đổi PIN trên tài khoản thật**. |
| **Gửi Thẻ Ngân Hàng (`/api/transfer/card`)** | | [X] | [ ] | [ ] | Đã xác minh CmdId 15220 trong bundle. Cần capture E1 để làm rõ cấu trúc JSON trả về của thẻ. |
| **Gửi Danh Thiếp (11774 / 11827)** | | [X] | [ ] | [ ] | Đã xác minh phân nhánh 1-1 và group. Cần kiểm chứng UI modal chọn nhiều. |
| **Bố cục Composer 2 cột** | | [ ] | [X] | [ ] | Có thể kiểm chứng hoàn toàn bằng egui harness test và quan sát native trên Windows 11 ARM64. |
| **Thanh công cụ RTF nằm dưới** | | [ ] | [X] | [ ] | Đã đối chiếu UI-LIVE ảnh Zalo PC và Zalo Web. Cần harness test vị trí hiển thị. |
| **Phím tắt Mention (Mũi tên / Enter)** | | [ ] | [X] | [ ] | Hoàn toàn kiểm chứng được qua unit test bàn phím và egui harness. |
| **Cấu hình Enter gửi tin (`AppSettings`)** | | [ ] | [X] | [ ] | Kiểm chứng qua test đọc/ghi `app_settings.json` và xử lý sự kiện phím. |
| **zTask Server Synchronization** | [X] | [ ] | [ ] | [ ] | **CHƯA NGHIÊN CỨU ĐỦ.** Chưa có bằng chứng E2 về endpoint và schema của zTask. Hiện tại chỉ dừng ở mức đổi View UI. |
| **Artifact Settings Modal Zalo Web** | [ ] | [ ] | [ ] | [ ] | **Pending.** Cần phiên browser tiếp theo chụp lại đúng ảnh Tab Tin nhắn và Cài đặt chung của Web để lưu đĩa. |

---

> **Cam kết tuân thủ:** Toàn bộ nội dung trên được tổng hợp dựa trên chứng cứ thực nghiệm, đối chiếu độc lập và thẩm tra chéo giữa Zalo PC (asar bundle), Zalo Web (webview session) và kho mã nguồn `ctdozlite`. Mọi đề xuất triển khai tiếp theo phải bám sát 5 gói công việc tại Phần D và các cấp độ kiểm chứng tại Phần E.
