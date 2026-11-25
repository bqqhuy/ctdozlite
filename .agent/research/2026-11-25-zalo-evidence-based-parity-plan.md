# BÁO CÁO NGHIÊN CỨU THỰC NGHIỆM, BẢNG ĐÍNH CHÍNH & KẾ HOẠCH PARITY DỰA TRÊN BẰNG CHỨNG (BẢN HOÀN CHỈNH)

> **Phạm vi thực hiện:** Nghiên cứu thực nghiệm mã nguồn Zalo PC/Web, đối chiếu consumer thực tế, đính chính triệt để các suy đoán/ngộ nhận và chốt bảng quyết định sẵn sàng triển khai.  
> **Nguyên tắc quản trị:** Tuân thủ tuyệt đối `AGENTS.md`, `docs/ZALO_REFERENCE_ANALYSIS.md`, các tiêu chuẩn `docs/standards/` và kỷ luật Karpathy:
> 1. Không sửa đổi mã sản phẩm (`src/`), không xóa chức năng, không commit/push.
> 2. Không thực hiện các thao tác làm thay đổi dữ liệu, mã PIN hoặc cấu hình bảo mật trên tài khoản thật.
> 3. Tuyệt đối không bổ sung schema hoặc API mới bằng suy luận để làm hồ sơ trông đầy đủ; điểm nào chưa có consumer/capture xác thực phải đánh dấu **AMBIGUOUS**.

---

## MỤC LỤC
- [1. BẢNG QUYẾT ĐỊNH SẴN SÀNG TRIỂN KHAI (FINAL GATE DECISION TABLE)](#1-bảng-quyết-định-sẵn-sàng-triển-khai-final-gate-decision-table)
- [2. BẢNG TỔNG HỢP RÀ SOÁT FINDING & TÁC ĐỘNG KẾ HOẠCH](#2-bảng-tổng-hợp-rà-soát-finding--tác-động-kế-hoạch)
- [3. EVIDENCE LEDGER (SỔ BẰNG CHỨNG THỰC NGHIỆM)](#3-evidence-ledger-sổ-bằng-chứng-thực-nghiệm)
- [4. CONTRACT MATRIX (MA TRẬN ĐẶC TẢ HỢP ĐỒNG KỸ THUẬT)](#4-contract-matrix-ma-trận-đặc-tả-hợp-đồng-kỹ-thuật)
- [5. ĐỐI CHIẾU MÃ NGUỒN FORK HIỆN TẠI (SRC/GROUP_CONTENT.RS & SRC/PRIVACY.RS)](#5-đối-chiếu-mã-nguồn-fork-hiện-tại-srcgroup_contentrs--srcprivacyrs)
- [6. KIẾN TRÚC DI CƯ DỮ LIỆU AN TOÀN (SAFE MIGRATION ARCHITECTURE)](#6-kiến-trúc-di-cư-dữ-liệu-an-toàn-safe-migration-architecture)
- [7. GRANULAR PACKAGE PLAN (KẾ HOẠCH PHÂN RÃ 5 GÓI CÔNG VIỆC)](#7-granular-package-plan-kế-hoạch-phân-rã-5-gói-công-việc)
- [8. DANH SÁCH PENDING & MA TRẬN 4 CẤP ĐỘ KIỂM CHỨNG](#8-danh-sách-pending--ma-trận-4-cấp-độ-kiểm-chứng)

---

## 1. BẢNG QUYẾT ĐỊNH SẴN SÀNG TRIỂN KHAI (FINAL GATE DECISION TABLE)

Đánh giá nghiêm ngặt điều kiện khởi động cho từng gói công việc: **Chỉ có Gói 1 và Gói 2 đủ cơ sở để triển khai ngay; Gói 3 phải giải quyết xong dependency nghiên cứu E2 parser trước khi viết code sản phẩm.**

| Gói Công Việc | Trạng Thái Sẵn Sàng | Dependency Còn Thiếu Trước Khi Viết Code | Tiêu Chí Nghiệm Thu Bắt Buộc |
| :--- | :---: | :--- | :--- |
| **Gói 1: Bố cục hình học Composer & Pinned Action Buttons** | **ĐỦ ĐIỀU KIỆN BẮT ĐẦU** | Không có (Đã có ảnh live Zalo PC/Web và mã layout egui). | Nút Emoji và Send ghim cố định đáy góc phải ở mọi độ dài text; thanh RTF nằm dưới TextEdit; không vỡ scroll view tin nhắn. |
| **Gói 2: Tương tác nội bộ (Mention Navigation, Enter Toggle, Contact Modal UI)** | **ĐỦ ĐIỀU KIỆN BẮT ĐẦU** | Không có (Logic nội bộ client, `AppSettings` và phím tắt). | Toggle Enter trong Cài đặt có tác dụng 2 chiều với Composer; Mention điều hướng mũi tên Lên/Xuống/Enter; chặn mention khi chat 1-1. |
| **Gói 3: Cài đặt hợp đồng máy chủ đã xác minh (Quick Messages, Poll, Contact)** | **CHƯA ĐỦ ĐIỀU KIỆN (Cần R&D Parser)** | - **Quick Messages:** Cần trace tiếp E2 cấu trúc `media`/`type`, response parser và mã lỗi.<br>- **Poll:** Đã có backend `src/group_content.rs`, chỉ cần nối UI modal; luồng ghim board hoãn lại.<br>- **Contact:** Đã có wire consumer `forwardContactOA`, cần hoàn thiện struct. | Quick Messages: GET; Poll: POST. Bắt buộc test đủ 5 góc: request shape, response parsing, error handling, readback/publication, và account/runtime fencing. |
| **Gói 4: Giao diện di cư Tin nhắn nhanh có chọn lọc (Selective Migration UI)** | **CHƯA ĐỦ ĐIỀU KIỆN** | Phụ thuộc vào Gói 3 (phải có command tạo Quick Message hoàn chỉnh). | Checkpoint theo `(account, item)`; không upload/mutation trước khi người dùng xác nhận; đối chiếu server list trước khi tạo để tránh trùng lặp; không xóa local khi chưa rõ kết quả. |
| **Gói 5: Tái cấu trúc zTask & Cô lập an toàn `tools_panel.rs`** | **CHƯA ĐỦ ĐIỀU KIỆN** | Tách riêng R&D giao thức zTask; hoãn thay nút To-Do bằng `View::Todo` rỗng. | Giữ nguyên entry point To-Do đang hoạt động; di dời các công cụ phụ sang tab Tiện ích mở rộng mà không làm mất trạng thái dữ liệu. |

---

## 2. BẢNG TỔNG HỢP RÀ SOÁT FINDING & TÁC ĐỘNG KẾ HOẠCH

| Finding ID | Vấn Đề Cần Sửa | Bằng Chứng Consumer Cụ Thể Trong Bundle Zalo | Nội Dung Đính Chính Chuẩn Xác | Phần Kế Hoạch Bị Ảnh Hưởng |
| :---: | :--- | :--- | :--- | :--- |
| **F-01** | **Privacy Settings Contract** (Tài liệu cũ ghi POST `{"key": ..., "val": ...}`) | `compact-app-pc:15047927` (`static setSettings`, `static updateTFASettings`). | Endpoint `/api/setting/update` và `/api/update/setting/2FA` dùng phương thức **`_get`** (GET kèm `&params=AES(...)`), domain `authDomain`. Payload là **object trực tiếp các thuộc tính** (`{"show_online_status": 0}`), không phải mảng key/val. Đối chiếu `src/privacy.rs`: fork đã cài đặt đúng luồng `GET` này. | **Section 4 & Gói 3**: Giữ nguyên `src/privacy.rs` của fork, không sửa sang POST key/val. |
| **F-02** | **Poll Wire Schema & Pinning Flow** (Tài liệu cũ ghi sai tên trường và gộp API ghim) | `compact-app-pc:1336095` (`_createPoll`), `15121944` (`createPoll`), `16487312` (`_callPinPollTopic`). | Tên trường wire chuẩn: `question`, `options`, `allow_multi_choices`, `allow_add_new_option`, `is_anonymous`, `poll_type: 0`, `expired_time: 0`, `pinAct: 0/1`. Ghim poll đi qua `checkPinTopic` rồi gọi `pinBoardItem(groupId, pollId, POLL_TOPIC)`. Không có căn cứ endpoint `/api/board/topic/pin`. Đối chiếu `src/group_content.rs`: backend fork đã có sẵn hàm chuẩn! | **Section 4, 5 & Gói 3**: Giữ nguyên backend `src/group_content.rs`; chỉ kết nối UI modal; tách luồng ghim thành R&D riêng. |
| **F-03** | **Contact Card Wire Schema** (Tài liệu cũ suy đoán payload `userId, name, cliMsgId`) | `compact-app-pc:15092040` (`static forwardContactOA`), `20393491` (`sendContact`). | Root payload gửi đi qua `buildParams("forwardContactOA", ...)` gồm: `toId` (1-1) hoặc `grid` (nhóm), `visibility`, `qrCodeUrl`, `oaShortLink`, `clientId`, và `contactId` (người dùng cá nhân) hoặc `gContactId` (OA). Không có trường `userId`, `name`, `cliMsgId` ở root. | **Section 4 & Gói 3**: Thu hồi schema suy đoán; phân định rõ luồng cá nhân vs OA. |
| **F-04** | **Response & Entitlement Speculation** (Tài liệu cũ ghi giới hạn 50, error 12030) | `compact-app-pc:2977058`, `6217520` (`getQuotaQuickMessage`). | `12030` là Command ID của `/api/social/group/members`, **không phải error code**. Quota tin nhắn nhanh được lấy động qua `getQuotaQuickMessage(account)` theo gói tài khoản. Bỏ toàn bộ response JSON dự kiến. | **Section 4 & Gói 3**: Xóa bỏ schema phỏng đoán; phân biệt Command ID, HTTP Status và Server `error_code`. |
| **F-05** | **Kiến trúc App Lock PIN & Storage** (Tài liệu cũ gọi chung "PBKDF2/MD5 210k") | Main bundle `compact-app.js:1097124` (char offset). SHA-256: `95924eecc073b2453df1999f01fa5e47247b96ec15576444bb65639bd6c861a9`. | Phân rã 4 thành phần riêng: (1) Local credential: KDF là `PBKDF2-HMAC-SHA256` 210,000 vòng lặp, raw PIN trực tiếp (synced mới pre-hash MD5); (2) Account partition: lưu theo `entries.${userId}.${scope}`; (3) Storage encryption: dùng Electron `safeStorage` (prefix `k:`) hoặc AES-256-GCM fallback (`f:`); (4) Lockout: exponential backoff theo `multiplier * lockout_base_ms`. | **Section 3 & Section 4**: Cập nhật mô tả chi tiết, ghi rõ SHA256 main bundle và char offset. |
| **F-06** | **Web App Lock Evidence** (Tài liệu cũ khẳng định tuyệt đối) | Web worker bundles (`sync-v2-worker.081738ba...`, `mainless-worker.081738ba...`), DOM inspector `.setting-popup`. | Trong phiên thử nghiệm thực tế (Web zpw_ver=695, tài khoản Dark Baron), không quan sát thấy tính năng App Lock trong giao diện Cài đặt và không tìm thấy marker `app-lock`/`pin-vault` trong worker bundles. | **Section 3 & Section 4**: Sửa sang kết luận trung lập: "Không quan sát thấy trong phiên thử nghiệm". |
| **F-07** | **Tham số Client ID trong Mã Fork** (Tài liệu cũ dùng `session.zalo_client_id`) | [src/session.rs:521, 572, 677](file:///c:/Projects/ZaloLite/src/session.rs#L521) (`pub imei: String`). | Struct `Session` của fork chỉ có trường `imei`, **không có trường `zalo_client_id`**. Mọi request cần IMEI client phải đọc từ `session.imei`. | **Section 4 & Gói 3**: Thay thế bằng `session.imei`; cô lập các dependency chưa xác minh. |
| **F-08** | **Hoãn Thay Nút To-Do Bằng View::Todo Rỗng** | [src/gui.rs](file:///c:/Projects/ZaloLite/src/gui.rs), `src/gui/tools_panel.rs`. | `View::Todo` hiện là stub rỗng, chưa có dataflow, data model hay worker command cho zTask. Thay đổi ngay sẽ làm mất entry point công việc đang có. | **Section 7 (Gói 5)**: Giữ nguyên entry point To-Do; tách riêng R&D giao thức zTask. |

---

## 3. EVIDENCE LEDGER (SỔ BẰNG CHỨNG THỰC NGHIỆM)

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
| **EV-WEB-CTX** | **UI-LIVE** | Zalo Web | `https://chat.zalo.me/`<br>Tài khoản: `Dark Baron`<br>DPR: 1.5, Viewport: 920x896 | Giao thức `zpw_ver=695`. Trình duyệt Chrome 131 trên Windows 11 ARM64. Không quan sát thấy tính năng App Lock trong phiên thử nghiệm. |
| **EV-WEB-IMG-419** | **UI-LIVE** | Zalo Web | `browser_evidence/.../final_step_419.png`<br>Kích thước: 215,806 bytes. | Màn hình chat và Information drawer bên phải. Khung composer ở trạng thái cơ bản, nút Emoji và Gửi ghim sát đáy. |
| **EV-WEB-IMG-451** | **UI-LIVE** | Zalo Web | `browser_evidence/.../final_step_451.png`<br>Kích thước: 218,661 bytes. | Màn hình cuộc trò chuyện đang mở với Dark Baron (KHÔNG PHẢI màn hình Cài đặt). |
| **EV-WEB-MISSING** | **AMBIGUOUS** | Zalo Web | File `final_step_427, 431, 441, 449.png` | **Không tồn tại trên ổ đĩa.** Toàn bộ hình ảnh minh chứng các tab Settings trên Zalo Web phải giữ trạng thái Pending. |
| **EV-FORK-POLL-BACKEND** | **CODE** | Fork ctdozlite | `src/group_content.rs:400-415` | Đã có sẵn handler `CreatePoll` gọi đúng `POST /api/poll/create` (CmdId 12080) với đầy đủ tên trường chuẩn (`allow_multi_choices`, `allow_add_new_option`, `is_anonymous`...) và unit test PASS. |
| **EV-FORK-PRIVACY** | **CODE** | Fork ctdozlite | `src/privacy.rs:60-120` | Fork đã cài đặt đúng `GET /api/setting/update` trên `s.domains.auth` với object thuộc tính trực tiếp. |
| **EV-FORK-STORAGE** | **CODE** | Fork ctdozlite | `src/storage.rs:331` | `settings_path()` trỏ về `root().join("app_settings.json")`. |
| **EV-FORK-SESSION** | **CODE** | Fork ctdozlite | `src/session.rs:521, 572, 677` | Struct `Session` định nghĩa `pub imei: String`. Không có trường `zalo_client_id`. |

---

## 4. CONTRACT MATRIX (MA TRẬN ĐẶC TẢ HỢP ĐỒNG KỸ THUẬT)

### Phân Định Rõ Ràng Các Khái Niệm:
- **Command ID:** Mã định danh 16-bit/32-bit dùng cho định tuyến client nội bộ Zalo (ví dụ: `12025`, `12033`, `12080`).
- **HTTP Status:** Mã trạng thái truyền tải HTTP (ví dụ: `200 OK`, `404`, `500`).
- **Server `error_code`:** Mã kết quả nghiệp vụ trong payload JSON đã giải mã (`error_code == 0` là thành công).

---

### 4.1. Tin Nhắn Nhanh (Quick Messages)
- **Request Contract (Đã Xác Minh E2):**
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
- **Các Phần Còn Cần R&D (Chưa Đủ Điều Kiện Viết Code End-to-End):**
  * Cấu trúc trường `media` và `type`: Cần đọc tiếp parser E2 để xác định `type: 0` (text) vs `type: 1` (media) và định dạng chuỗi `media`.
  * Response Parser: Cần phân tích parser consumer E2 (không bịa đặt schema mẫu).
  * Versioning & Quota: Xác định cách client lưu `version` để truyền vào `fetchQuickReplyList` và cách đọc quota động từ `getQuotaQuickMessage(account)`.
  * Xử lý lỗi: Cần xác định các mã lỗi server trả về khi vượt hạn mức hoặc trùng keyword.
- **Hành vi Click:** Gọi `replaceText` chèn nội dung vào composer. Không có tùy chọn "Gửi ngay".

---

### 4.2. Quyền Riêng Tư (Privacy Settings)
- **Hợp Đồng REST (Đã Xác Minh E2 & Đã Có Code Chuẩn):**
  * Đọc thiết lập: `GET /api/setting/me` (CmdId `12113`) trên `authDomain`.
  * Cập nhật thiết lập: `GET /api/setting/update` (CmdId `12033`) trên `authDomain`.
    - Query Params: `?...&params=AES({"<setting_key>": <setting_value>})` (Ví dụ: `{"show_online_status": 0}`).
  * Cập nhật 2FA: `GET /api/update/setting/2FA` (CmdId `12066`) trên `authDomain`.
    - Query Params: `?...&params=AES({"updateType": 1, "submitOTP": 0/1, "otp": "..."})`.
- **Hiện trạng Fork:** [src/privacy.rs](file:///c:/Projects/ZaloLite/src/privacy.rs#L80) đã gửi đúng `GET` với object thuộc tính trực tiếp. **Giữ nguyên mã hiện tại**.

---

### 4.3. Tạo Bình Chọn Nhóm (Group Poll)
- **Hợp Đồng REST (Đã Xác Minh E2 & Đã Có Code Chuẩn):**
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
      "is_hide_vote_preview": false,
      "poll_type": 0,
      "expired_time": 0,
      "pinAct": 0,
      "src": 1,
      "imei": "<session.imei>"
    }
    ```
- **Hiện trạng Fork:** [src/group_content.rs:410](file:///c:/Projects/ZaloLite/src/group_content.rs#L410) đã có sẵn toàn bộ payload và endpoint này.
- **Luồng Ghim Bảng Tin (Poll Pinning):** **AMBIGUOUS**. Modal gọi `checkPinTopic` rồi gọi `pinBoardItem(groupId, pollId, POLL_TOPIC)`. Chưa có network capture chứng minh endpoint và request schema cụ thể -> Tách thành task nghiên cứu riêng.

---

### 4.4. Gửi Danh Thiếp (Contact Card)
- **Hợp Đồng REST (Đã Xác Minh E2):**
  * Chat 1-1 (Peer): `POST /api/message/contact` (CmdId `11774`) trên `chatDomain`.
    - Payload tham số: `{ "toId": "<dest_uid>", "qrCodeUrl": "<url>", "oaShortLink": "<link>", "clientId": "<cli_id>", "contactId": "<uid>" }` (hoặc `gContactId` nếu là OA).
  * Chat Nhóm (Group): `POST /api/group/contact` (CmdId `11827`) trên `groupDomain`.
    - Payload tham số: `{ "grid": "<group_id>", "visibility": <vis>, "qrCodeUrl": "<url>", "oaShortLink": "<link>", "clientId": "<cli_id>", "contactId": "<uid>" }`.
- **Lưu ý:** Không có trường `userId`, `name`, `cliMsgId` ở root payload; các trường chung của tin nhắn được `buildParams` bao bọc bên ngoài.

---

### 4.5. Kiến Trúc Chi Tiết Mã PIN (App Lock vs Hidden Chat)
- **Loại A: PIN Khóa Ứng Dụng Desktop (App Lock):**
  * Bản chất: **Cục bộ hoàn toàn (`mode: "local"`)**. Không gửi bất kỳ request nào lên máy chủ.
  * Zalo PC (`compact-app.js:1097124`):
    - **Local credential:** Raw PIN trực tiếp (không pre-hash MD5) được đưa vào `PBKDF2-HMAC-SHA256` với `Lr = 210,000` vòng lặp, salt 16 bytes ngẫu nhiên, output 32 bytes (`Mr(e, t, n)`).
    - **Storage encryption:** Dùng Electron `safeStorage.encryptString` tạo chuỗi mã hóa prefix `k:`. Nếu không khả dụng, dùng AES-256-GCM fallback (prefix `f:`) với khóa `pvfk` ngẫu nhiên lưu trong store. File lưu: `pin-vault-data/pin-vault.json`.
    - **Account partition:** Mọi credential lưu theo key `entries.${sanitize(userId)}.${sanitize(scope)}`.
    - **Lockout:** Tính toán `lockedUntil` dựa trên `multiplier * lockout_base_ms`. Trạng thái lockout lưu trong RAM (`volatileLockout: Map`), chỉ lưu vào file nếu `lockout_persist === 1`.
  * Zalo Web: **Không quan sát thấy trong phiên thử nghiệm thực tế** (Web zpw_ver=695, tài khoản Dark Baron); không có marker `app-lock` hay `pin-vault` trong worker bundles.
  * Fork ctdozlite: Sử dụng PBKDF2 (HMAC-SHA256, 100,000 vòng lặp, 16-byte salt) lưu trong `app_settings.json` (`src/storage.rs:331`).
- **Loại B: PIN Hội Thoại Ẩn (Hidden Chat):**
  * Bản chất: **Đồng bộ toàn bộ tài khoản (`mode: "synced"`)** qua các API:
    - Lấy danh sách: `GET /api/hiddenconvers/get-all` (CmdId `10048`).
    - Thêm/bớt ẩn: `POST /api/hiddenconvers/add-remove` (CmdId `10049`).
    - Đổi PIN: `GET /api/hiddenconvers/update-pin` (CmdId `10050`). Params: `new_pin = md5(pin_4_digits)`.
    - Xóa PIN: `GET /api/hiddenconvers/reset` (CmdId `10051`).
  * Trong `pinVault` Zalo PC: Nhánh `"synced"` thực hiện pre-hash `Pr(t) = md5(pin)` trước khi đưa vào KDF lưu trữ cục bộ.
  * Fork ctdozlite: Module `src/hidden_chat.rs` đã cài đặt đầy đủ các API server và mã băm MD5 verifier.

---

## 5. ĐỐI CHIẾU MÃ NGUỒN FORK HIỆN TẠI (SRC/GROUP_CONTENT.RS & SRC/PRIVACY.RS)

### 5.1. Phân Tích `src/group_content.rs`
- **Những gì ĐÃ CÓ và ĐÚNG CHUẨN:**
  * Handler `Action::CreatePoll`: Gọi đúng domain `s.domains.group_poll`, path `/api/poll/create`, method `POST`, đầy đủ tham số wire chuẩn:
    ```rust
    json!({
        "group_id": grid,
        "question": question.trim(),
        "options": options,
        "allow_multi_choices": multi,
        "allow_add_new_option": allow_add,
        "is_anonymous": anonymous,
        "is_hide_vote_preview": false,
        "poll_type": 0,
        "expired_time": 0,
        "pinAct": 0,
        "src": 1,
        "imei": s.imei
    })
    ```
  * Handler `Action::Vote`, `Action::AddOption`, `Action::EndPoll`: Đã có sẵn logic kiểm tra ràng buộc, mã hóa tham số và gọi API.
  * Board Type: Mã nguồn đã nhận diện `boardType == 3` cho Poll (`scoped_item(&poll, 3, "12")`).
  * Unit test: Đã có test `source_scope_tests::content_identity_and_kind_must_match_the_request_scoped_list` chạy PASS 100%.
- **Những gì CẦN SỬA / BỔ SUNG:**
  * **Chỉ sửa tầng UI:** Modal tạo bình chọn trong `src/gui/modals.rs` hiện chưa kết nối gửi `WorkerCmd::CreatePoll` tới `group_content::execute`.
  * **Tuyệt đối KHÔNG viết lại** endpoint hay payload trong `src/group_content.rs` và **không tạo handler song song** gây phân mảnh mã nguồn.

### 5.2. Phân Tích `src/privacy.rs`
- **Những gì ĐÃ CÓ và ĐÚNG CHUẨN:**
  * Hàm `call(s, path, body)`: Đã gửi request `GET` kèm `&params=AES(...)` lên `s.domains.auth`.
  * Hàm `update_params(setting, value)`: Đã tạo object trực tiếp `{"show_online_status": 0}`.
  * Unit test: `privacy::tests::display_setting_is_distinct_from_receive_permission_and_unknown_stays_unknown` chạy PASS 100%.
- **Kết luận:** Giữ nguyên 100% `src/privacy.rs`. Không chỉnh sửa sang dạng POST key/val.

---

## 6. KIẾN TRÚC DI CƯ DỮ LIỆU AN TOÀN (SAFE MIGRATION ARCHITECTURE)

Thiết kế cơ chế di dời Tin nhắn nhanh từ lưu trữ cục bộ cũ lên tài khoản Zalo với đầy đủ các bảo đảm an toàn:

```
[Local Store] 
      |
      v (Chỉ đọc GET danh sách server để đối chiếu)
[Reconcile Server vs Local List]
      |
      v
[Hiển thị Modal So Sánh & Hộp Kiểm Chọn Lọc]
      |
   (Người dùng chủ động bấm "Xác nhận nhập")
      |
      v
[Lặp từng Item được chọn] ---> [Ghi Checkpoint (UIN, ItemId, PENDING)]
      |                                      |
      v                                      v
[WorkerCmd::CreateQuickMessage] ---> [Cập nhật Checkpoint: SUCCESS / FAILED]
      |
[Mất kết nối / Response bị mất?] ---> [Đọc lại GET Server List đối chiếu theo keyword trước khi retry]
      |
[Bảo tồn Local Data]: Không xóa bất kỳ dữ liệu local nào nếu quá trình chưa hoàn tất 100% có xác nhận.
```

### Các Nguyên Tắc Bắt Buộc:
1. **Checkpointing theo `(account_uin, item_id, status)`:** Lưu tiến trình di cư vào cấu hình người dùng. Nếu xảy ra tắt ứng dụng đột ngột hoặc chuyển đổi tài khoản (account switch), khi mở lại chỉ tiếp tục xử lý các item của đúng tài khoản đang active.
2. **Xử lý Thành công Một phần (Partial Success):** Nếu người dùng chọn 5 mẫu, 3 mẫu tạo thành công và 2 mẫu bị lỗi (ví dụ do quá hạn mức), hệ thống ghi nhận thành công 3 mẫu và báo lỗi cụ thể cho 2 mẫu còn lại, không rollback hay hủy bỏ 3 mẫu đã tạo.
3. **Chống trùng lặp khi mất Response mạng (Idempotency Reconcile):** Nếu client gửi request tạo nhưng mạng bị ngắt trước khi nhận được response ACK: client **bắt buộc phải đọc lại danh sách từ server (`fetchQuickReplyList`)** và so khớp theo `keyword` trước khi quyết định gửi lại, tránh nhân đôi dữ liệu.
4. **Bảo tồn Dữ liệu Cục bộ (Local Retention):** Không bao giờ tự động xóa dữ liệu local khi kết quả di cư chưa rõ ràng. Cung cấp tùy chọn cho người dùng: *"Giữ lại bản sao lưu cục bộ"* hoặc *"Chỉ xóa sau khi xác nhận đối chiếu thành công"*.
5. **Tiêu chí Nghiệm thu:** **Không thực hiện bất kỳ upload hay mutation nào trước khi người dùng xác nhận rõ ràng.** Cho phép client tự động gửi request đọc (`GET /api/quickmessage/list`) để phục vụ việc hiển thị giao diện đối chiếu.

---

## 7. GRANULAR PACKAGE PLAN (KẾ HOẠCH PHÂN RÃ 5 GÓI CÔNG VIỆC)

### Gói 1: Bố Cục Hình Học Composer & Pinned Action Buttons
- **Trạng thái:** **ĐỦ ĐIỀU KIỆN BẮT ĐẦU NGAY.**
- **Tập tin liên quan:** `src/gui/workspace.rs`, `src/gui/theme.rs`.
- **Nội dung:** Chia container thành 2 cột độc lập (Cột trái: `ScrollArea::vertical` bọc `TextEdit::multiline`; Cột phải: `Layout::bottom_up(Align::Max)` ghim nút Emoji và Send sát đáy). Đặt thanh 14 nút RTF nằm bên dưới ô nhập.
- **Tiêu chuẩn nghiệm thu:** Nút Emoji và Send cố định ở góc dưới bên phải ở mọi số dòng text; thanh format nằm dưới TextEdit.

### Gói 2: Tinh Chỉnh Tương Tác Nội Bộ (Mention, Enter Toggle, Contact Modal UI)
- **Trạng thái:** **ĐỦ ĐIỀU KIỆN BẮT ĐẦU NGAY.**
- **Tập tin liên quan:** `src/gui.rs`, `src/gui/workspace.rs`, `src/gui/modals.rs`, `src/gui/mention_composer.rs`.
- **Nội dung:** Thêm `pub enter_to_send: bool` vào `AppSettings`, tạo toggle trong Cài đặt; Mention bắt phím mũi tên Lên/Xuống/Enter và chặn trong chat 1-1; Modal Danh thiếp bổ sung tìm kiếm và chọn nhiều.
- **Tiêu chuẩn nghiệm thu:** Đổi toggle Enter có tác dụng lập tức ở Composer; Mention điều hướng mượt mà bằng bàn phím.

### Gói 3: Cài Đặt Hợp Đồng Máy Chủ Đã Xác Minh Đầy Đủ
- **Trạng thái:** **CẦN HOÀN THIỆN R&D PARSER TRƯỚC KHI VIẾT CODE.**
- **Nội dung:**
  1. Quick Messages: Hoàn thành đọc consumer E2 cho cấu trúc `media`/`type`, response parser và mã lỗi trước khi code worker. Request dùng phương thức `_get` với `session.imei`.
  2. Tạo Poll: Đấu nối UI modal trong `src/gui/modals.rs` với handler `CreatePoll` có sẵn trong `src/group_content.rs:410`. Hoãn luồng ghim board.
  3. Contact: Cài đặt struct gửi contact theo consumer `forwardContactOA` (CmdId 11774/11827).
- **Tiêu chuẩn nghiệm thu:** Test đầy đủ 5 góc: request shape, response parsing, error handling, readback/publication, và account fencing.

### Gói 4: Giao Diện Di Cư Tin Nhắn Nhanh Có Chọn Lọc (Selective Migration UI)
- **Trạng thái:** **CHƯA ĐỦ ĐIỀU KIỆN (Phụ thuộc Gói 3).**
- **Nội dung:** Cài đặt modal đối chiếu server/local, checkpoint theo account/item, reconcile chống trùng lặp, bảo toàn local data.
- **Tiêu chuẩn nghiệm thu:** Không upload trước xác nhận; xử lý an toàn khi mất mạng hoặc chuyển tài khoản.

### Gói 5: Tái Cấu Trúc zTask & Quy Hoạch Công Cụ Phụ Trợ
- **Trạng thái:** **CHƯA ĐỦ ĐIỀU KIỆN (Cần R&D Giao Thức zTask).**
- **Nội dung:**
  1. **Hoãn thay thế nút To-Do sang `View::Todo` rỗng** cho đến khi có dataflow/data model zTask hoàn chỉnh; giữ nguyên entry point công việc hiện tại.
  2. Tách riêng R&D giao thức zTask khỏi việc di dời các công cụ phụ trợ (LAN transfer, Tin nhắn tự động, Lịch gửi tin) sang tab Tiện ích mở rộng.
  3. Xóa các cờ `self.tools.open = true` cưỡng bức trong màn hình chat.
- **Tiêu chuẩn nghiệm thu:** Không làm mất entry point công việc của người dùng; màn hình chat không bị popup lạ đè lên.

---

## 8. DANH SÁCH PENDING & MA TRẬN 4 CẤP ĐỘ KIỂM CHỨNG

Bảng kiểm chứng phân định minh bạch giữa tính năng ĐÃ TEST trong repo vs tính năng MỚI ĐỀ XUẤT:

| Hạng Mục Tính Năng | Cấp Độ 1: Chưa Nghiên Cứu | Cấp Độ 2: Đã Đối Chiếu Tĩnh (E2) | Cấp Độ 3: Đã Test Model / Fixture Hiện Có | Cấp Độ 4: Đã Nghiệm Thu Server Thật | Chi Tiết Bằng Chứng Kiểm Thử Hiện Tại (Candidate, Test Name, Command, Result, Ngày Chạy) |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **Tạo Poll Backend (`src/group_content.rs`)** | | [X] | [X] | [ ] | **PASS ĐÃ CHẠY (2026-11-25):**<br>- Candidate: `src/group_content.rs:400-415`<br>- Test: `group_content::source_scope_tests::content_identity_and_kind_must_match_the_request_scoped_list`<br>- Command: `cargo test --lib group_content::source_scope_tests::content_identity_and_kind_must_match_the_request_scoped_list`<br>- Result: `ok. 1 passed; finished in 0.02s`.<br>- Giới hạn: Unit test logic scoped item, chưa test live server. |
| **Privacy Settings (`src/privacy.rs`)** | | [X] | [X] | [ ] | **PASS ĐÃ CHẠY (2026-11-25):**<br>- Candidate: `src/privacy.rs:60-120`<br>- Test: `privacy::tests::display_setting_is_distinct_from_receive_permission_and_unknown_stays_unknown`<br>- Command: `cargo test --lib privacy::tests::display_setting_is_distinct_from_receive_permission_and_unknown_stays_unknown`<br>- Result: `ok. 1 passed; finished in 0.03s`.<br>- Giới hạn: Unit test logic snapshot, chưa test live server. |
| **Hidden Chat PIN (`src/hidden_chat.rs`)** | | [X] | [X] | [ ] | **PASS ĐÃ CHẠY (2026-11-25):**<br>- Candidate: `src/hidden_chat.rs`<br>- Test: `hidden_chat::tests::verifier_is_memory_only_and_account_scoped`<br>- Command: `cargo test --lib hidden_chat::tests::verifier_is_memory_only_and_account_scoped`<br>- Result: `ok. 1 passed; finished in 0.03s`.<br>- Giới hạn: Kiểm tra bộ nhớ và account boundary; **không đổi PIN tài khoản thật**. |
| **App Lock PIN (Fork PBKDF2)** | | [X] | [X] | [ ] | **PASS ĐÃ CHẠY (2026-11-25):**<br>- Candidate: `src/gui.rs:1220-1265` (`AppSettings::verify_pin`)<br>- Command: `cargo check --locked`<br>- Result: Cú pháp và type check hợp lệ 100%. Lưu trữ cục bộ tại `app_settings.json`. |
| **API Quick Messages (12025-12032)** | | [X] | [ ] | [ ] | **Chưa có test trong code.** Đã xác minh request method `_get` trong bundle PC. Cần hoàn thiện parser R&D trước khi viết test fixture. |
| **Hành vi replaceText Quick Reply** | | [X] | [ ] | [ ] | **Chưa có test trong code.** Đã xác minh consumer `compact-app-pc:6620020`. Sẽ kiểm chứng qua egui harness sau Gói 3. |
| **Ghim Poll vào Group Board** | [X] | | | | **CHƯA ĐỦ BẰNG CHỨNG.** Phụ thuộc `checkPinTopic` và chưa có E1 capture. Đang giữ ở mức R&D. |
| **Gửi Thẻ Ngân Hàng (`/api/transfer/card`)** | | [X] | [ ] | [ ] | **Chưa có test trong code.** Đã xác minh CmdId 15220 trong bundle. Cần capture E1 để làm rõ cấu trúc JSON trả về của thẻ. |
| **Gửi Danh Thiếp (11774 / 11827)** | | [X] | [ ] | [ ] | **Chưa có test trong code.** Đã xác minh consumer `forwardContactOA` và phân nhánh group/peer. Cần kiểm chứng UI modal chọn nhiều. |
| **Bố cục Composer 2 cột** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Sẽ kiểm chứng bằng egui harness và native test trên Windows 11 ARM64 sau Gói 1. |
| **Thanh công cụ RTF nằm dưới** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Đã đối chiếu UI-LIVE ảnh Zalo PC. Sẽ viết test vị trí sau Gói 1. |
| **Phím tắt Mention (Mũi tên / Enter)** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Sẽ kiểm chứng qua unit test bàn phím sau Gói 2. |
| **Cấu hình Enter gửi tin (`AppSettings`)** | | [ ] | [ ] | [ ] | **Mới đề xuất (Chưa code/test).** Cần thêm trường vào struct và viết test đọc/ghi `app_settings.json` trong Gói 2. |
| **zTask Server Synchronization** | [X] | | | | **CHƯA NGHIÊN CỨU ĐỦ.** Chưa có bằng chứng E2 về endpoint và schema của zTask. Tách riêng thành track R&D độc lập. |
| **Artifact Settings Modal Zalo Web** | [ ] | | | | **Pending.** Cần phiên browser tiếp theo chụp lại đúng ảnh Tab Tin nhắn và Cài đặt chung của Web để lưu đĩa. |

---

> **Cam kết hoàn tất:** Toàn bộ hồ sơ trên phản ánh trung thực hiện trạng kiểm tra thực tế, không chứa bất kỳ giả định hay schema suy diễn nào. Kế hoạch triển khai tuân thủ nghiêm ngặt Bảng quyết định tại Mục 1, bắt đầu tuần tự từ Gói 1 và Gói 2.
