# BÁO CÁO NGHIÊN CỨU THỰC NGHIỆM, BẢNG ĐÍNH CHÍNH & KẾ HOẠCH PARITY DỰA TRÊN BẰNG CHỨNG (PHIÊN BẢN RÀ SOÁT CHUẨN XÁC)

> **Phạm vi thực hiện:** Nghiên cứu thực nghiệm mã nguồn Zalo PC/Web, đối chiếu consumer thực tế, đính chính triệt để các suy đoán/ngộ nhận và cập nhật kế hoạch parity.  
> **Nguyên tắc quản trị:** Tuân thủ tuyệt đối `AGENTS.md`, `docs/ZALO_REFERENCE_ANALYSIS.md`, các tiêu chuẩn `docs/standards/` và kỷ luật Karpathy:
> 1. Không sửa đổi mã sản phẩm (`src/`), không xóa chức năng, không commit/push.
> 2. Không thực hiện các thao tác làm thay đổi dữ liệu, mã PIN hoặc cấu hình bảo mật trên tài khoản thật.
> 3. Tuyệt đối không bổ sung schema hoặc API mới bằng suy luận để làm hồ sơ trông đầy đủ; điểm nào chưa có consumer/capture xác thực phải đánh dấu **AMBIGUOUS**.

---

## BẢNG TỔNG HỢP RÀ SOÁT FINDING & TÁC ĐỘNG KẾ HOẠCH

Bảng bàn giao đối chiếu trực tiếp giữa các finding cần sửa, bằng chứng consumer cụ thể trong bundle, nội dung đính chính và phần kế hoạch bị ảnh hưởng:

| Finding ID | Vấn Đề Cần Sửa | Bằng Chứng Consumer Cụ Thể Trong Bundle Zalo | Nội Dung Đính Chính Chuẩn Xác | Phần Kế Hoạch Bị Ảnh Hưởng |
| :---: | :--- | :--- | :--- | :--- |
| **F-01** | **Privacy Settings Contract** (Tài liệu cũ ghi POST `{"key": ..., "val": ...}`) | `compact-app-pc:15047927` (`static setSettings`, `static updateTFASettings`). | Endpoint `/api/setting/update` và `/api/update/setting/2FA` dùng phương thức **`_get`** (GET với `&params=AES(...)`), domain `authDomain`. Payload là **object trực tiếp các thuộc tính** (`{"show_online_status": 0}`), không phải mảng key/val. Đối chiếu `src/privacy.rs`: fork đã cài đặt đúng luồng `GET` này. | **Section C (Contract Matrix) & Gói 3**: Giữ nguyên `src/privacy.rs` của fork, không sửa đổi sang POST key/val. |
| **F-02** | **Poll Wire Schema & Pinning Flow** (Tài liệu cũ ghi sai tên trường và gộp API ghim) | `compact-app-pc:1336095` (`_createPoll`), `15121944` (`createPoll`), `16487312` (`_callPinPollTopic`). | Tên trường wire chuẩn: `question`, `options`, `allow_multi_choices`, `allow_add_new_option`, `is_anonymous`, `poll_type: 0`, `expired_time: 0`, `pinAct: 0/1`. Ghim poll đi qua `checkPinTopic` và `pinBoardItem(groupId, pollId, POLL_TOPIC)`. Không có căn cứ endpoint `/api/board/topic/pin`. | **Section C & Gói 3**: Sửa struct request Poll đúng wire schema; đánh dấu hợp đồng ghim bảng tin là **AMBIGUOUS / Pending nghiên cứu**. |
| **F-03** | **Contact Card Wire Schema** (Tài liệu cũ suy đoán payload `userId, name, cliMsgId`) | `compact-app-pc:15092040` (`static forwardContactOA`), `20393491` (`sendContact`). | Root payload gửi đi qua `buildParams("forwardContactOA", ...)` gồm: `toId` (1-1) hoặc `grid` (nhóm), `visibility`, `qrCodeUrl`, `oaShortLink`, `clientId`, và `contactId` (người dùng cá nhân) hoặc `gContactId` (OA). Không có trường `userId`, `name`, `cliMsgId` ở root. | **Section C & Gói 3**: Thu hồi schema suy đoán; phân định rõ luồng cá nhân vs OA. |
| **F-04** | **Response & Entitlement Speculation** (Tài liệu cũ ghi giới hạn 50, error 12030) | `compact-app-pc:2977058`, `6217520` (`getQuotaQuickMessage`). | `12030` là Command ID của `/api/social/group/members`, **không phải error code**. Quota tin nhắn nhanh được lấy động qua `getQuotaQuickMessage(account)` theo gói tài khoản. | **Section C & Section E**: Xóa bỏ schema phỏng đoán và error code giả lập; đánh dấu **AMBIGUOUS**. Phân định rõ Command ID, HTTP Status và Server `error_code`. |
| **F-05** | **Kiến trúc App Lock PIN & Storage** (Tài liệu cũ gọi chung "PBKDF2/MD5 210k") | Main bundle `compact-app.js:1097124` (char offset). SHA256: `95924eecc073b2453df1999f01fa5e47247b96ec15576444bb65639bd6c861a9`. | Phân rã 4 thành phần riêng: (1) Local credential: KDF là `PBKDF2-HMAC-SHA256` 210,000 vòng lặp, raw PIN trực tiếp (synced mới pre-hash MD5); (2) Account partition: lưu theo `entries.${userId}.${scope}`; (3) Storage encryption: dùng Electron `safeStorage` (prefix `k:`) hoặc AES-256-GCM fallback (`f:`); (4) Lockout: exponential backoff theo `multiplier * lockout_base_ms`. | **Section B & Section C**: Cập nhật mô tả chi tiết, ghi rõ SHA256 main bundle và char offset. Xác nhận Web không có App Lock dựa trên cả bundle worker và DOM tree. |
| **F-06** | **Tính Xác Thực Artifact & Cấp Độ Test** (Tài liệu cũ dẫn `final_step_451.png` làm Settings; đánh PASS tính năng chưa làm) | Filesystem thư mục `browser_evidence/`, `cargo test --lib`. | `final_step_451.png` là màn hình chat, không phải Settings -> Chuyển toàn bộ Settings Web thành **Pending**. Bỏ toàn bộ dấu `[X]` (PASS) khỏi các tính năng đề xuất. Chỉ ghi PASS cho các bài test thực sự tồn tại trong repo. | **Section B & Section E**: Khôi phục tính minh bạch của ma trận bằng chứng và kiểm chứng. |
| **F-07** | **Kế Hoạch Triển Khai & Struct Session** (Tài liệu cũ dùng `session.zalo_client_id`) | `src/session.rs:521, 572, 677` (`pub imei: String`). | Struct `Session` của fork chỉ có trường `imei`, **không có trường `zalo_client_id`**. Gói 3 bị thu hẹp: chỉ triển khai các contract đã xác minh đầy đủ; các phần chưa rõ phải có task nghiên cứu độc lập trước. | **Section D (Granular Plan)**: Thay thế bằng `session.imei`; cô lập các dependency chưa xác minh. |

---

## A. BẢNG ĐÍNH CHÍNH TOÀN DIỆN

Bảng tổng hợp tất cả các điểm đính chính kỹ thuật đối chiếu với thực tế kiểm tra:

| STT | Vấn Đề Kỹ Thuật | Nhận Định Cũ Trong Tài Liệu Trước | Kết Luận Kiểm Chứng Thực Tế | Bằng Chứng Xác Minh (E1/E2/CODE/UI) | Lý Do & Tác Động Triển Khai |
| :---: | :--- | :--- | :--- | :--- | :--- |
| **1** | **Phương thức HTTP Tin nhắn nhanh (Quick Messages)** | Ghi nhận create, update, delete dùng `POST` với payload `shortcut`, `attachments`. | **SAI HOÀN TOÀN.** Cả 4 API đều dùng phương thức `_get` (GET với `&params=AES(...)`). | **E2**: `compact-app-pc:15161790`: `fetchQuickReplyList` (12025), `createQuickReplyItem` (12029), `updateQuickReplyItem` (12026), `deleteQuickReplyItem` (12032). | Sửa đặc tả worker: Gửi request `_get` với param cipher AES; tên trường là `keyword`, `message`, `media`, `type`, `imei`. |
| **2** | **Hành vi click chọn Tin nhắn nhanh** | Cho rằng có tùy chọn hoặc nút bấm "Gửi ngay" trực tiếp từ popover. | **BÁC BỎ.** Click vào một mẫu tin nhắn nhanh luôn thay thế/chèn vào ô soạn thảo văn bản (`replaceText`). Không có tùy chọn "Gửi ngay". | **E2**: `compact-app-pc:6620020`: `replaceText: o.wrap("quick-message", l.replaceText)`. | Bỏ thiết kế cờ `send_immediately` khỏi UI popover; chỉ thực hiện chèn nội dung vào composer. |
| **3** | **Phương thức & Schema Quyền riêng tư (Privacy)** | Ghi nhận POST `/api/setting/update` với payload `{"key": ..., "val": ...}`. | **SAI HOÀN TOÀN.** API dùng phương thức `_get` trên `authDomain`, payload là object trực tiếp các thuộc tính (`{"show_online_status": 0}`). | **E2**: `compact-app-pc:15047927` (`static setSettings`, CmdId 12033). **CODE**: `src/privacy.rs:80` của fork đã cài đặt đúng luồng `GET` này. | Giữ nguyên implementation chuẩn của `src/privacy.rs`, không sửa thành POST key/val. |
| **4** | **Wire Schema Tạo bình chọn (Poll)** | Dùng tên trường suy đoán (`multi_choice`, `allow_add_option`, `hide_vote`). | **SAI TÊN TRƯỜNG.** Tên trường chuẩn: `allow_multi_choices`, `allow_add_new_option`, `is_anonymous`, `poll_type: 0`, `expired_time: 0`, `pinAct: 0/1`. | **E2**: `compact-app-pc:1336095` (`_createPoll`), `compact-app-pc:15121944` (`createPoll`, CmdId 12080 trên `group_poll`). | Cập nhật struct tham số trong `src/group_content.rs` khớp 100% tên trường wire chính thức. |
| **5** | **Luồng Ghim Bình Chọn (Poll Pinning)** | Cho rằng có endpoint xác minh `/api/board/topic/pin` với `boardType = 3`. | **CHƯA ĐỦ BẰNG CHỨNG.** Luồng ghim đi qua modal check quyền `checkPinTopic` rồi gọi `pinBoardItem(groupId, pollId, POLL_TOPIC)`. Chưa có endpoint capture. | **E2**: `compact-app-pc:16487312`. | Đánh dấu hợp đồng ghim bình chọn là **AMBIGUOUS**. Tách riêng việc tạo poll và ghim poll thành 2 bước. |
| **6** | **Wire Schema Gửi Danh Thiếp (Contact)** | Ghi nhận payload gửi gồm `userId`, `name`, `cliMsgId` lên `/api/message/contact`. | **SAI LỆCH.** Root payload không chứa `userId`, `name`, `cliMsgId`. Gồm: `toId`/`grid`, `visibility`, `qrCodeUrl`, `oaShortLink`, `clientId`, `contactId`/`gContactId`. | **E2**: `compact-app-pc:15092040` (`forwardContactOA`, CmdId 11774 1-1, CmdId 11827 nhóm). | Xóa bỏ schema suy đoán; định nghĩa struct khớp chính xác consumer `forwardContactOA`. |
| **7** | **Response Schemas & Entitlement Limits** | Ghi nhận limit 50, error code 12030 cho Quick Messages và schema response giả định. | **BÁC BỎ.** 12030 là Command ID của social members. Limit Quick Messages phụ thuộc quota tài khoản (`getQuotaQuickMessage`). Schema response chưa có E1 capture. | **E2**: `compact-app-pc:2977058`, `6217520`. | Đánh dấu toàn bộ response schemas và limits chưa có parser là **AMBIGUOUS**. |
| **8** | **Kiến trúc Mã PIN App Lock (Local Vault)** | Gọi chung là "PBKDF2/MD5 210k" và suy đoán fork giống Zalo PC. | **CHÍNH XÁC HÓA.** App Lock dùng raw PIN trực tiếp (không qua MD5) với PBKDF2-HMAC-SHA256 210k vòng lặp. Lưu trữ qua Electron `safeStorage` (hoặc AES-GCM), phân vùng theo `userId`. Fork dùng PBKDF2 100k trong `app_settings.json`. | **E2**: `compact-app.js:1097124` (main bundle). **CODE**: `src/storage.rs:331`. | Làm rõ sự khác biệt giữa chuẩn Zalo PC (`pinVault`) và triển khai hiện tại của fork (`app_settings.json`). |
| **9** | **Tính Xác Thực Artifact Zalo Web** | Dẫn `final_step_451.png` làm bằng chứng Settings và trích dẫn các file 427, 431, 441, 449. | **BÁC BỎ.** `final_step_451.png` là màn hình chat. Các file 427, 431, 441, 449 không tồn tại trên ổ đĩa. | **FS CHECK**: Thư mục `browser_evidence/...` chỉ có 419 và 451 (đều là chat view). | Đánh dấu toàn bộ bằng chứng hình ảnh Settings Web là **Chưa có artifact / Pending**. |
| **10** | **Tham số Client ID trong Mã Fork** | Đề xuất dùng `session.zalo_client_id`. | **SAI STRUCT.** Struct `Session` trong `src/session.rs` chỉ có trường `pub imei: String`. | **CODE**: `src/session.rs:521, 572, 677`. | Đính chính: Mọi request cần IMEI client phải đọc từ `session.imei`. |
| **11** | **Vị trí Thanh Định Dạng RTF (Composer)** | Khẳng định thanh định dạng RTF `Aa` "bắt buộc phải nằm ở TRÊN CÙNG của khung soạn thảo". | **BÁC BỎ.** Cả Zalo PC và Zalo Web đều đặt thanh công cụ định dạng **ở DƯỚI CÙNG** của ô nhập văn bản khi bật RTF. | **UI-LIVE**: `zalo_pc_composer_live.png` và Zalo Web live. | Bố trí thanh format nằm dưới TextEdit; mở rộng chiều cao composer lên trên. |
| **12** | **Cấu hình Phím Enter Gửi Tin trong Fork** | Cho rằng "Đã có trường cấu hình trong AppSettings và chỉ cần nối UI". | **SAI LỆCH.** Struct `AppSettings` trong `src/gui.rs:1075-1137` **hoàn toàn chưa có** trường `enter_to_send` hay `send_on_enter`. `workspace.rs:5362` hardcode Enter. | **CODE**: `src/gui.rs:1075-1137` và `src/gui/workspace.rs:5359-5362`. | Phải bổ sung trường `enter_to_send: bool` vào `AppSettings`, tạo UI toggle trong tab Tin nhắn và cập nhật logic bắt phím trong `workspace.rs`. |

---

## B. EVIDENCE LEDGER (SỔ BẰNG CHỨNG THỰC NGHIỆM)

| Mã Định Danh | Loại Bằng Chứng | Môi Trường | Nguồn / Đường Dẫn / Định Vị | Kết Luận Thực Nghiệm & Đo Đạc Kỹ Thuật |
| :--- | :---: | :---: | :--- | :--- |
| **EV-PC-RENDER-BUNDLE** | **E2** | Zalo PC 26.11.12 | `%TEMP%\zalo-asar-analysis\pc-dist\compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>SHA-256: `78801014154a86bade096eb916e541223a5f553f888882aa0ef3a218a44d6c08`<br>Độ dài: `21,621,214` ký tự (UTF-8). | Toàn bộ routing, consumer API renderer, modal components và hằng số giao thức. |
| **EV-PC-MAIN-BUNDLE** | **E2** | Zalo PC 26.11.12 | `%TEMP%\zalo-asar-analysis\main-dist\compact-app.js`<br>SHA-256: `95924eecc073b2453df1999f01fa5e47247b96ec15576444bb65639bd6c861a9`<br>Độ dài: `4,029,463` ký tự (UTF-8). | Định nghĩa `pinVault` tại **char offset `1,097,124`**. Quản lý credential, safeStorage, account partition và lockout. |
| **EV-PC-QR-API** | **E2** | Zalo PC 26.11.12 | `compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Char offset: `15,161,790` - `15,162,200` | 4 static method: `fetchQuickReplyList` (CmdId 12025), `createQuickReplyItem` (CmdId 12029), `updateQuickReplyItem` (CmdId 12026), `deleteQuickReplyItem` (CmdId 12032). Tất cả đều là `_get` với `&params=...`. |
| **EV-PC-QR-ACTION** | **E2** | Zalo PC 26.11.12 | `compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Char offset: `6,620,020` | `replaceText: o.wrap("quick-message", l.replaceText)`: Click mẫu tin nhắn nhanh thay thế text composer; không có tùy chọn gửi ngay. |
| **EV-PC-PRIVACY** | **E2** | Zalo PC 26.11.12 | `compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Char offset: `15,047,927` | `setSettings`: `_get` đến `/api/setting/update` (CmdId 12033), params là object thuộc tính trực tiếp. `updateTFASettings`: `_get` đến `/api/update/setting/2FA` (CmdId 12066). |
| **EV-PC-POLL** | **E2** | Zalo PC 26.11.12 | `compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Char offset: `1,336,095` (Modal) & `15,121,944` (API) | `_createPoll` tạo object wire: `question, options, allow_multi_choices, allow_add_new_option, is_anonymous, poll_type: 0, expired_time: 0, pinAct`. `createPoll` gửi `POST /api/poll/create` (CmdId 12080). |
| **EV-PC-CONTACT** | **E2** | Zalo PC 26.11.12 | `compact-app-pc.0cd2e653e67efaa4bf4b.js`<br>Char offset: `15,092,040` (`forwardContactOA`) | Phân nhánh: 1-1 gửi `/api/message/contact` (CmdId 11774); Nhóm gửi `/api/group/contact` (CmdId 11827). Payload bọc qua `buildParams`, chứa `toId`/`grid`, `visibility`, `qrCodeUrl`, `oaShortLink`, `clientId`, `contactId`/`gContactId`. |
| **EV-PC-COMP-UI** | **UI-LIVE** | Zalo PC 26.11.12 | `zalo_pc_composer_live.png`<br>Kích thước: 882,864 bytes. | Khung soạn thảo Zalo PC: Thanh công cụ định dạng RTF nằm ở DƯỚI ô nhập. Nút Like/Gửi và Emoji ghim cố định góc dưới bên phải. |
| **EV-WEB-CTX** | **UI-LIVE** | Zalo Web | `https://chat.zalo.me/`<br>Tài khoản: `Dark Baron`<br>DPR: 1.5, Viewport: 920x896 | Giao thức `zpw_ver=695`. Trình duyệt Chrome 131 trên Windows 11 ARM64. Không có tính năng App Lock trong bundle/DOM. |
| **EV-WEB-IMG-419** | **UI-LIVE** | Zalo Web | `browser_evidence/.../final_step_419.png`<br>Kích thước: 215,806 bytes. | Màn hình chat và Information drawer bên phải. Khung composer ở trạng thái cơ bản, nút Emoji và Gửi ghim sát đáy. |
| **EV-WEB-IMG-451** | **UI-LIVE** | Zalo Web | `browser_evidence/.../final_step_451.png`<br>Kích thước: 218,661 bytes. | Màn hình cuộc trò chuyện đang mở với Dark Baron (KHÔNG PHẢI màn hình Cài đặt). |
| **EV-WEB-MISSING** | **AMBIGUOUS** | Zalo Web | File `final_step_427, 431, 441, 449.png` | **Không tồn tại trên ổ đĩa.** Toàn bộ hình ảnh minh chứng các tab Settings trên Zalo Web phải giữ trạng thái Pending. |
| **EV-FORK-PRIVACY** | **CODE** | Fork ctdozlite | `src/privacy.rs:60-120` | Fork đã cài đặt đúng `GET /api/setting/update` trên `s.domains.auth` với object thuộc tính trực tiếp. |
| **EV-FORK-STORAGE** | **CODE** | Fork ctdozlite | `src/storage.rs:331` | `settings_path()` trỏ về `root().join("app_settings.json")` (không phải `settings.json`). |
| **EV-FORK-SESSION** | **CODE** | Fork ctdozlite | `src/session.rs:521, 572, 677` | Struct `Session` định nghĩa `pub imei: String`. Không có trường `zalo_client_id`. |

---

## C. CONTRACT MATRIX (MA TRẬN ĐẶC TẢ HỢP ĐỒNG KỸ THUẬT)

### Nguyên Tắc Phân Định Khái Niệm:
- **Command ID:** Mã định danh 16-bit/32-bit dùng cho định tuyến nội bộ của client Zalo (ví dụ: `12025`, `12033`, `12080`). Không phải HTTP status và không phải error code.
- **HTTP Status:** Mã phản hồi tầng truyền tải HTTP (ví dụ: `200 OK`, `404 Not Found`, `500 Internal Server Error`).
- **Server `error_code`:** Mã kết quả nghiệp vụ nằm trong thân JSON giải mã từ máy chủ (`error_code == 0` là thành công, khác 0 là lỗi nghiệp vụ).

---

### 1. Chi Tiết Từng Hợp Đồng Trọng Yếu:

#### 1.1. Tin Nhắn Nhanh (Quick Messages)
- **Caller UI:** Nút `⚡` trên Composer; mục "Quản lý tin nhắn nhanh" trong Cài đặt.
- **Hợp Đồng REST (E2 Parity - Đã Xác Minh):**
  * `fetchQuickReplyList(version=0, lang=0)`:
    - Domain: `getQuickMessageDomain()`
    - Route & Method: `GET /api/quickmessage/list`
    - CmdId: `12025`
    - Query Params: `?...&params=AES({"version": e, "lang": t, "imei": session.imei})`
  * `createQuickReplyItem({ keyword, message, media, type })`:
    - Route & Method: `GET /api/quickmessage/create`
    - CmdId: `12029`
    - Query Params: `?...&params=AES({"keyword": k, "message": m, "media": med, "type": typ, "imei": session.imei})`
  * `updateQuickReplyItem({ id, keyword, message, media, type })`:
    - Route & Method: `GET /api/quickmessage/update`
    - CmdId: `12026`
    - Query Params: `?...&params=AES({"itemId": id, "keyword": k, "message": m, "media": med, "type": typ})`
  * `deleteQuickReplyItem(itemIds)`:
    - Route & Method: `GET /api/quickmessage/delete`
    - CmdId: `12032`
    - Query Params: `?...&params=AES({"itemIds": [...]})`
- **Response Schema:** **AMBIGUOUS** (chưa có live capture E1; dự kiến chứa wrapper `{ error_code: 0, data: { items: [...], version: ... } }`).
- **Giới hạn & Error Code:** **AMBIGUOUS** (Hạn mức lấy động qua `getQuotaQuickMessage(account)` tùy gói; không có căn cứ hardcode 50 hay error 12030).
- **Hành vi Click:** Gọi `replaceText` chèn nội dung vào composer. Không có tùy chọn "Gửi ngay".

#### 1.2. Quyền Riêng Tư (Privacy Settings)
- **Caller UI:** Tab "Quyền riêng tư" trong Cài đặt.
- **Hợp Đồng REST (E2 Parity - Đã Xác Minh):**
  * Đọc thiết lập: `GET /api/setting/me` (CmdId `12113`) trên `authDomain`.
  * Cập nhật thiết lập: `GET /api/setting/update` (CmdId `12033`) trên `authDomain`.
    - Query Params: `?...&params=AES({"<setting_key>": <setting_value>})` (Ví dụ: `{"show_online_status": 0}`).
  * Cập nhật 2FA: `GET /api/update/setting/2FA` (CmdId `12066`) trên `authDomain`.
    - Query Params: `?...&params=AES({"updateType": 1, "submitOTP": 0/1, "otp": "..."})`.
- **Đối chiếu mã fork:** `src/privacy.rs` đã gửi đúng `GET /api/setting/update` với object thuộc tính trực tiếp. **Giữ nguyên mã hiện tại**.

#### 1.3. Tạo Bình Chọn Nhóm (Group Poll)
- **Caller UI:** Menu `...` trên Composer -> "Tạo bình chọn".
- **Hợp Đồng REST (E2 Parity - Đã Xác Minh):**
  * Route & Method: `POST /api/poll/create`
  * CmdId: `12080`
  * Domain: `getGroupPollDomain()`
  * Body payload (AES mã hóa):
    ```json
    {
      "group_id": "<conv_id_without_g>",
      "question": "<string>",
      "options": ["<opt1>", "<opt2>"],
      "allow_multi_choices": true,
      "allow_add_new_option": true,
      "is_anonymous": false,
      "poll_type": 0,
      "expired_time": 0,
      "pinAct": 0,
      "src": 1,
      "imei": "<session.imei>"
    }
    ```
- **Luồng Ghim Bảng Tin (Poll Pinning):** **AMBIGUOUS** (Client kiểm tra modal `checkPinTopic` và gọi `pinBoardItem(groupId, pollId, POLL_TOPIC)`. Chưa có network capture chứng minh endpoint và request schema cụ thể).

#### 1.4. Gửi Danh Thiếp (Contact Card)
- **Caller UI:** Nút Danh thiếp trên Composer.
- **Hợp Đồng REST (E2 Parity - Đã Xác Minh):**
  * Chat 1-1 (Peer): `POST /api/message/contact` (CmdId `11774`) trên `chatDomain`.
    - Payload tham số: `{ "toId": "<dest_uid>", "qrCodeUrl": "<url>", "oaShortLink": "<link>", "clientId": "<cli_id>", "contactId": "<uid>" }` (hoặc `gContactId` nếu là OA).
  * Chat Nhóm (Group): `POST /api/group/contact` (CmdId `11827`) trên `groupDomain`.
    - Payload tham số: `{ "grid": "<group_id>", "visibility": <vis>, "qrCodeUrl": "<url>", "oaShortLink": "<link>", "clientId": "<cli_id>", "contactId": "<uid>" }`.
- **Lưu ý:** Không có trường `userId`, `name`, `cliMsgId` ở root payload; các trường chung của tin nhắn được `buildParams` bao bọc bên ngoài.

#### 1.5. Thẻ Tài Khoản Ngân Hàng (Bank Card)
- **Hợp Đồng REST:** `POST /api/transfer/card` (CmdId `15220`) trên `getZInstantMsgDomain()`.
- **Payload Schema & Response:** **AMBIGUOUS** (Cần E1 capture để xác nhận wire format thực tế trước khi implement).

#### 1.6. Kiến Trúc Chi Tiết Mã PIN (App Lock vs Hidden Chat)
- **Loại A: PIN Khóa Ứng Dụng Desktop (App Lock):**
  * Bản chất: **Cục bộ hoàn toàn (`mode: "local"`)**. Không gửi bất kỳ request nào lên máy chủ.
  * Zalo PC (`compact-app.js:1097124`):
    - **Local credential:** Raw PIN trực tiếp (không pre-hash MD5) được đưa vào `PBKDF2-HMAC-SHA256` với `Lr = 210,000` vòng lặp, salt 16 bytes ngẫu nhiên, output 32 bytes (`Mr(e, t, n)`).
    - **Storage encryption:** Dùng Electron `safeStorage.encryptString` tạo chuỗi mã hóa prefix `k:`. Nếu không khả dụng, dùng AES-256-GCM fallback (prefix `f:`) với khóa `pvfk` ngẫu nhiên lưu trong store. File lưu: `pin-vault-data/pin-vault.json`.
    - **Account partition:** Mọi credential lưu theo key `entries.${sanitize(userId)}.${sanitize(scope)}`.
    - **Lockout:** Tính toán `lockedUntil` dựa trên `multiplier * lockout_base_ms`. Trạng thái lockout lưu trong RAM (`volatileLockout: Map`), chỉ lưu vào file nếu `lockout_persist === 1`.
  * Zalo Web: **HOÀN TOÀN KHÔNG CÓ**. Đã xác minh qua phân tích bundle worker và kiểm tra cấu trúc DOM thực tế (không chỉ dựa vào việc thiếu screenshot).
  * Fork ctdozlite: Sử dụng PBKDF2 (HMAC-SHA256, 100,000 vòng lặp, 16-byte salt) lưu trong `app_settings.json` (`src/storage.rs:331`). Phù hợp bản chất local của desktop nhưng khác biệt về định dạng file lưu trữ.
- **Loại B: PIN Hội Thoại Ẩn (Hidden Chat):**
  * Bản chất: **Đồng bộ toàn bộ tài khoản (`mode: "synced"`)** qua các API:
    - Lấy danh sách: `GET /api/hiddenconvers/get-all` (CmdId `10048`).
    - Thêm/bớt ẩn: `POST /api/hiddenconvers/add-remove` (CmdId `10049`).
    - Đổi PIN: `GET /api/hiddenconvers/update-pin` (CmdId `10050`). Params: `new_pin = md5(pin_4_digits)`.
    - Xóa PIN: `GET /api/hiddenconvers/reset` (CmdId `10051`).
  * Trong `pinVault` Zalo PC: Nhánh `"synced"` thực hiện pre-hash `Pr(t) = md5(pin)` trước khi đưa vào KDF lưu trữ cục bộ.
  * Fork ctdozlite: Module `src/hidden_chat.rs` đã cài đặt đầy đủ các API server và mã băm MD5 verifier.

---

## D. GRANULAR PACKAGE PLAN (KẾ HOẠCH PHÂN RÃ 5 GÓI CÔNG VIỆC)

```
+-----------------------------------------------------------------------------------+
| GÓI 1: Bố Cục Hình Học Composer & Pinned Action Buttons (UI Cốt Lõi)              |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| GÓI 2: Tinh Chỉnh Tương Tác Nội Bộ (Mention Navigation, Enter Toggle, Contact UI) |
+-----------------------------------------------------------------------------------+
                                          |
                                          v
+-----------------------------------------------------------------------------------+
| GÓI 3: Cài Đặt Hợp Đồng Máy Chủ Đã Xác Minh (Quick Messages GET, Poll Wire Schema)|
| * Lưu ý: Các hợp đồng chưa đủ E1 (Bank Card, Poll Pinning) chuyển sang task R&D   |
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
- **Mục tiêu:** Chấm dứt lỗi nút Emoji và Like bị căn giữa trôi lơ lửng khi nhập nhiều dòng; bố trí thanh định dạng RTF nằm bên dưới ô nhập văn bản.
- **Tập tin liên quan:** `src/gui/workspace.rs`, `src/gui/theme.rs`.
- **Giải pháp kỹ thuật:**
  * Chia container soạn thảo thành 2 cột:
    - Cột trái (`ui.available_width() - action_w`): Chứa `egui::ScrollArea::vertical` bọc `TextEdit::multiline`.
    - Cột phải: Dùng layout dọc `with_layout(Layout::bottom_up(Align::Max))` để ghim nút Emoji và nút Send/Like luôn sát đáy góc dưới bên phải. Chiều rộng tính động theo kích thước nút và DPI (không hardcode 76.0px).
  * Khi `composer_format_mode == true`, render thanh 14 nút RTF **ngay bên dưới** vùng soạn thảo văn bản.
- **Tiêu chuẩn nghiệm thu:**
  - Nhập 1 dòng hoặc 20 dòng: Nút Emoji và Send luôn cố định ở góc dưới cùng bên phải.
  - Bật RTF: Thanh format xuất hiện phía dưới ô nhập văn bản.

---

### Gói 2: Tinh Chỉnh Tương Tác Nội Bộ (Mention, Enter Toggle, Contact Modal)
- **Mục tiêu:** Hoàn thiện phím tắt và kết nối cấu hình gửi tin.
- **Tập tin liên quan:** `src/gui.rs`, `src/gui/workspace.rs`, `src/gui/modals.rs`, `src/gui/mention_composer.rs`.
- **Giải pháp kỹ thuật:**
  * Thêm `pub enter_to_send: bool` (mặc định `true`) vào struct `AppSettings` trong `src/gui.rs`.
  * Đấu nối toggle trong Tab Tin nhắn (`src/gui/modals.rs`) với logic bắt phím `consume_composer_enter` trong `src/gui/workspace.rs`.
  * Mention Keyboard: Trong `src/gui/mention_composer.rs`, bắt phím mũi tên Lên/Xuống để di chuyển vệt sáng chọn mục, phím Enter/Tab để chèn tên vào composer. Chặn popup mention trong hội thoại 1-1.
  * Modal Danh thiếp: Bổ sung thanh tìm kiếm và checkbox chọn nhiều người.
- **Tiêu chuẩn nghiệm thu:**
  - Đổi toggle trong Cài đặt lập tức có tác dụng ở Composer (tắt toggle -> gõ Enter xuống dòng, Ctrl+Enter gửi).
  - Gõ `@` trong nhóm: Điều hướng được bằng phím mũi tên và ấn Enter chọn `@All` hoặc thành viên.

---

### Gói 3: Cài Đặt Hợp Đồng Máy Chủ Đã Xác Minh Đầy Đủ
- **Mục tiêu:** Triển khai các worker command đã có bằng chứng consumer E2 rõ ràng.
- **Tập tin liên quan:** `src/chat.rs`, `src/gui/worker.rs`, `src/group_content.rs`.
- **Phạm vi triển khai cụ thể:**
  1. **Tin nhắn nhanh (Quick Messages):**
     - Bổ sung `WorkerCmd::FetchQuickMessages`, `WorkerCmd::CreateQuickMessage { keyword, message, media, msg_type }`, `WorkerCmd::UpdateQuickMessage`, `WorkerCmd::DeleteQuickMessage`.
     - Phương thức: Toàn bộ dùng **`_get`** với query string `&params=AES(...)`. Sử dụng `session.imei` cho tham số `imei`.
  2. **Tạo bình chọn (Group Poll):**
     - Cập nhật struct tham số trong `src/group_content.rs:410` theo đúng wire schema: `question`, `options`, `allow_multi_choices`, `allow_add_new_option`, `is_anonymous`, `poll_type: 0`, `expired_time: 0`, `pinAct: 0/1`.
     - Request gửi `POST /api/poll/create` (CmdId `12080`) trên domain `group_poll`.
  3. **Cô lập các hợp đồng chưa đủ bằng chứng:**
     - Luồng ghim bình chọn (`pinBoardItem`) và gửi Thẻ ngân hàng (`/api/transfer/card`) được tách thành **Task Nghiên Cứu Độc Lập**, chưa đưa vào đợt triển khai mã này cho đến khi có network capture E1.
- **Tiêu chuẩn nghiệm thu:**
  - `cargo check --locked` hoàn tất 0 lỗi.
  - Unit test mock giải mã query params xác nhận request gửi đi là phương thức GET và đúng cấu trúc AES.

---

### Gói 4: Giao Diện Di Cư Tin Nhắn Nhanh Có Chọn Lọc (Selective Migration UI)
- **Mục tiêu:** Cho phép người dùng chủ động đối chiếu và đưa tin nhắn nhanh từ cấu hình local cũ lên tài khoản Zalo mà không tự ý ghi đè.
- **Tập tin liên quan:** `src/gui/modals.rs`, `src/gui/workspace.rs`.
- **Giải pháp kỹ thuật:**
  * Tuyệt đối không tự động upload khi khởi động.
  * Khi mở màn hình quản lý tin nhắn nhanh: Nếu phát hiện có dữ liệu local cũ chưa đồng bộ, hiển thị banner đối chiếu.
  * Modal hiển thị danh sách 2 cột kèm checkbox: Người dùng chọn các mục cần tải lên -> Ứng dụng gọi `WorkerCmd::CreateQuickMessage`.
  * Cung cấp tùy chọn lưu trữ cục bộ (offline) nếu người dùng không muốn đồng bộ lên máy chủ.
- **Tiêu chuẩn nghiệm thu:**
  - Không có request mạng nào phát sinh nếu người dùng chưa bấm nút xác nhận nhập.

---

### Gói 5: Tái Cấu Trúc zTask & Cô Lập An Toàn `tools_panel.rs`
- **Mục tiêu:** Tách các phụ thuộc vào `tools_panel.rs`, chuẩn hóa nút "✓" và menu `...`.
- **Tập tin liên quan:** `src/gui.rs`, `src/gui/workspace.rs`, `src/gui/tools_panel.rs`.
- **Giải pháp kỹ thuật:**
  * Nút "✓" trên Navigation rail: Chuyển sang `View::Todo`. Giao diện thông báo rõ trạng thái dịch vụ zTask.
  * Gỡ bỏ các cờ mở popup `self.tools.open = true` trong `workspace.rs`.
  * Menu `...` trên Composer: Chỉ hiển thị các mục chuẩn Zalo (Bình chọn, Giao việc, Nhắc hẹn).
  * Di dời các tính năng tiện ích phụ trợ (LAN transfer, Tin nhắn tự động, Lịch gửi tin) vào tab Tiện ích mở rộng riêng biệt trong Cài đặt; tiến tới cô lập an toàn tệp `tools_panel.rs`.
- **Tiêu chuẩn nghiệm thu:**
  - Không còn bất kỳ popup lạ nào đè lên màn hình chat khi nhấp các nút chuẩn Zalo.

---

## E. DANH SÁCH PENDING & MA TRẬN 4 CẤP ĐỘ KIỂM CHỨNG

Bảng kiểm chứng phân định nghiêm ngặt giữa tính năng ĐÃ TEST trong repo vs tính năng MỚI ĐỀ XUẤT:

| Hạng Mục Tính Năng | Cấp Độ 1: Chưa Nghiên Cứu | Cấp Độ 2: Đã Đối Chiếu Tĩnh (E2) | Cấp Độ 3: Đã Test Model / Fixture Hiện Có | Cấp Độ 4: Đã Nghiệm Thu Server Thật | Ghi Chú & Bằng Chứng Kiểm Thử Hiện Tại |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **API Quick Messages (12025-12032)** | | [X] | [ ] | [ ] | Đã xác minh 4 hàm `_get` trong bundle PC. Cần viết fixture test mock response trước khi gọi thật. |
| **Hành vi replaceText Quick Reply** | | [X] | [ ] | [ ] | Đã xác minh consumer `compact-app-pc:6620020`. Chưa có harness test UI. |
| **Tạo Poll (`/api/poll/create` 12080)** | | [X] | [ ] | [ ] | Đã xác minh wire schema trong bundle PC và có sẵn trong `group_content.rs:410`. |
| **Ghim Poll vào Group Board** | [X] | | | | **CHƯA ĐỦ BẰNG CHỨNG.** Phụ thuộc `checkPinTopic` và chưa có E1 capture. Cần nghiên cứu thêm. |
| **Privacy Settings (`src/privacy.rs`)** | | [X] | [X] | [ ] | **PASS trong repo**: Test `privacy::tests::display_setting_is_distinct_from_receive_permission_and_unknown_stays_unknown` (`cargo test --lib privacy`). Giới hạn: Unit test logic, chưa test live server. |
| **Hidden Chat PIN (`src/hidden_chat.rs`)** | | [X] | [X] | [ ] | **PASS trong repo**: Đã có unit test giải mã MD5 verifier constant-time. **TUYỆT ĐỐI KHÔNG tự đổi PIN trên tài khoản thật**. |
| **App Lock PIN (Fork PBKDF2)** | | [X] | [X] | [ ] | **PASS trong repo**: `AppSettings::verify_pin` chạy qua PBKDF2 100k vòng lặp. Lưu trữ local trong `app_settings.json`. |
| **Gửi Thẻ Ngân Hàng (`/api/transfer/card`)** | | [X] | [ ] | [ ] | Đã xác minh CmdId 15220 trong bundle. Cần capture E1 để làm rõ cấu trúc JSON trả về của thẻ. |
| **Gửi Danh Thiếp (11774 / 11827)** | | [X] | [ ] | [ ] | Đã xác minh consumer `forwardContactOA` và phân nhánh group/peer. Cần kiểm chứng UI modal chọn nhiều. |
| **Bố cục Composer 2 cột** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Sẽ kiểm chứng bằng egui harness và native test trên Windows 11 ARM64 sau Gói 1. |
| **Thanh công cụ RTF nằm dưới** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Đã đối chiếu UI-LIVE ảnh Zalo PC. Sẽ viết test vị trí sau Gói 1. |
| **Phím tắt Mention (Mũi tên / Enter)** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Sẽ kiểm chứng qua unit test bàn phím sau Gói 2. |
| **Cấu hình Enter gửi tin (`AppSettings`)** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Cần thêm trường vào struct và viết test đọc/ghi `app_settings.json` trong Gói 2. |
| **zTask Server Synchronization** | [X] | | | | **CHƯA NGHIÊN CỨU ĐỦ.** Chưa có bằng chứng E2 về endpoint và schema của zTask. Hiện tại chỉ dừng ở mức đổi View UI. |
| **Artifact Settings Modal Zalo Web** | [ ] | | | | **Pending.** Cần phiên browser tiếp theo chụp lại đúng ảnh Tab Tin nhắn và Cài đặt chung của Web để lưu đĩa. |

---

> **Cam kết nghiệm thu:** Hồ sơ này loại bỏ hoàn toàn các giả định không có căn cứ, tuân thủ kỷ luật bằng chứng thực nghiệm của dự án. Mọi bước triển khai tiếp theo phải dựa trên các contract đã được xác minh tại Section C và kế hoạch phân rã tại Section D.
