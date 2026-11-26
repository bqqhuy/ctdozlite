# BÁO CÁO NGHIÊN CỨU THỰC NGHIỆM, BẢNG ĐÍNH CHÍNH & KẾ HOẠCH PARITY DỰA TRÊN BẰNG CHỨNG
*(Bản Hiệu Chỉnh 11:15 - Khắc Phục Triệt Để Hồi Quy, Đối Chiếu Thực Nghiệm Zalo Web chat.zalo.me & Desktop Bundle)*

> **Phạm vi tài liệu:** Báo cáo kỹ thuật tổng hợp phục vụ nghiên cứu thực nghiệm, phân tích kiến trúc và phản biện parity. Không thay thế các chuẩn mực quản trị hiện hành.
> **Nguyên tắc quản trị tối cao:** `AGENTS.md`, các tiêu chuẩn kỹ thuật tại `docs/standards/`, các quyết định kiến trúc (ADR) và hồ sơ Spec Kit là nguồn quản trị chuẩn mực căn bản:
> 1. Không sửa đổi mã sản phẩm (`src/`), không xóa chức năng, không commit/push.
> 2. Không thực hiện các thao tác làm thay đổi dữ liệu, mã PIN hoặc cấu hình bảo mật trên tài khoản thật. Không tự ý thực hiện mutation để tìm kiếm bằng chứng.
> 3. Phân định rạch ròi 5 tầng chứng cứ: **UI-LIVE** (quan sát trực tiếp giao diện/DOM), **E1** (network capture có request/response ID kiểm tra lại được), **E2** (thân hàm, hằng số trong bundle chính thức), **CODE** (mã fork hiện tại), **E3/AMBIGUOUS** (suy luận/chưa đủ chứng cứ).
> 4. Chỉ đánh dấu **PASS** cho các bài kiểm thử thực sự đã chạy và vượt qua trên candidate xác định (commit hash + fingerprint).

---

## 1. BẢNG SO SÁNH THAY ĐỔI SO VỚI BẢN 10:09 (HANDOVER CHANGE LOG)

| Nhóm Nội Dung | Hiện Trạng Bản 10:09 | Cập Nhật / Đính Chính Trong Bản 11:15 | Căn Cứ & Bằng Chứng Cụ Thể | Trạng Thái Dependency |
| :--- | :--- | :--- | :--- | :--- |
| **Gói 2B: Gửi Danh Thiếp** | Chỉ có E2 wire contract phân nhánh 1-1 vs Group (`forwardContactOA`). Chưa rõ giao diện chọn nhiều. | **Bổ sung UI-LIVE:** Modal 2 cột (`.zl-modal`), cột trái tìm kiếm + filter chips, cột phải khay chọn `Đã chọn: x/9` (hạn mức cứng tối đa 9 liên hệ), checkbox `Gửi kèm số điện thoại`. Tuần tự dispatch kèm `clientId` riêng biệt. | UI-LIVE Zalo Web (`final_step_523.png`); E2 `compact-app-pc:15092040`. | **MỞ:** Cần cài đặt UI modal 2 cột và luồng worker tuần tự có in-flight dedup. |
| **Gói Hợp Nhất Poll** | Đã có giải pháp 2 pha (mutation vs readback) và rào chắn UI-Worker fencing 2 đầu. | **GIỮ NGUYÊN TOÀN BỘ:** Tách outcome mutation (thành công khi có `poll_id`, readback lỗi chỉ cảnh báo tải lại) vs unknown outcome không tự retry. Fencing mang `owner`, `runtime_id`, `request_id`. | E2 `compact-app-pc:1336095`, `15121944`; CODE `src/group_content.rs:410`. | **SẴN SÀNG TRIỂN KHAI:** Triển khai độc lập sau khi bổ sung 4 regression tests trong code fork. |
| **Gói 3: Tin Nhắn Nhanh** | E2 xác nhận request method `_get`. Băn khoăn về hành vi chọn mẫu (chèn hay gửi ngay). | **Bổ sung UI-LIVE & E2:** Kích hoạt inline khi gõ `/` trong composer bật `.popover-v3.qrs-popover`. Bàn phím: Mũi tên Lên/Xuống, Enter để chèn, Esc để ẩn. **Hành vi chốt: Chèn văn bản vào composer, TUYỆT ĐỐI KHÔNG GỬI NGAY**. Ảnh đính kèm đưa vào preview tray (`onlyPreview: true`). Quota đọc động; "1/1" là quan sát thực tế trên tài khoản thử nghiệm Dark Baron (Free tier). | UI-LIVE (`final_step_564.png`); E1 Live Capture `quickmessage/list`; E2 `compact-app-pc:5005740`, `6131441`, `6720264`. | **MỞ:** Cần hoàn thiện parser response và cache SQLite mã hóa AES-CBC trước khi viết worker. |
| **Gói 4: Gửi Nhanh Số Tài Khoản (STK)** | Chưa nghiên cứu phân hệ STK (mới ghi nhận Command ID 15220). | **Bổ sung UI-LIVE & E2:** Nút số 8 trên composer toolbar. Khảo sát trọn bộ 6 API máy chủ: `conf` (15309), `list` (15298), `create` (15306), `update` (15307), `delete` (15308), `card` (15220). Regex STK `^[A-Za-z0-9]{1,19}$`, Tên chủ thẻ tự động viết hoa không dấu `^(?=.{5,50}$)[A-Za-z0-9]+(?: [A-Za-z0-9]+)*$`. Click thẻ gửi ngay qua `submitBankCardInfo`. | UI-LIVE (`final_step_509.png`); E1 Live Capture `transfer/list`; E2 `main-startup:2039310`, `compact-app-pc:15171516`. | **MỞ:** Phụ thuộc vào hoàn thiện mô hình dữ liệu STK trong fork. |
| **Gói 4: Kiến Trúc Di Cư Dữ Liệu (Migration)** | Thiết kế chi tiết 6 nguyên tắc an toàn, reconcile nội dung chuẩn hóa, checkpointing. | **KHÔI PHỤC ĐẦY ĐỦ:** Giữ nguyên 100% sơ đồ và 6 nguyên tắc di cư an toàn từ bản 10:09, không bị gộp mất hay tinh giản khi bổ sung STK. | Kiến trúc thiết kế dựa trên E2 consumer Zalo PC và chuẩn lưu trữ fork. | **MỞ:** Phụ thuộc vào Gói 3 (phải có command tạo Quick Message hoàn chỉnh). |
| **Gói 5: Menu `...` & Công Cụ zBusiness** | Đề xuất hoãn thay To-Do bằng stub rỗng và di dời tools. | **Bổ sung UI-LIVE:** Menu `...` toolbar composer gồm 5 mục: Tạo bình chọn, Tạo nhắc hẹn, Tạo ghi chú, Đánh dấu tin quan trọng (vàng), Đánh dấu tin khẩn cấp (đỏ). Nút "Công cụ" Left Rail là **"Công cụ zBusiness"** (`STR_BUSINESS_TOOL`) gồm 5 tiện ích: Tin nhanh, Danh mục, Trả lời tự động, Tin đánh dấu (Starred view), Tin đồng thời. zTask là dịch vụ webview độc lập. | UI-LIVE (`step_550_shot_1_toolbar_more_menu.png`, `cong_cu_view.png`). | **MỞ:** Tách riêng R&D giao thức zTask; giữ nguyên entry point To-Do hiện có. |
| **Bảng Mã Hóa & Ký Số (Crypto)** | Bản 10:38 khái quát hóa bảng AES-128 / signkey cho mọi request. | **ĐÍNH CHÍNH TRIỆT ĐỂ:** Bỏ bảng crypto tổng quát. Truy theo từng đường request thực tế. Tách biệt cơ chế bảo mật của Zalo (safeStorage, ParamCipher) khỏi thiết kế nội bộ fork (`app_settings.json` PBKDF2). | E2 decompiled bundles; CODE fork `src/session.rs`, `src/storage.rs`. | Không còn giả định sai. |

---

## 2. BẢNG QUYẾT ĐỊNH SẴN SÀNG TRIỂN KHAI (GATE DECISION TABLE)

| Gói Công Việc | Trạng Thái Sẵn Sàng | Dependency Còn Mở Trước Khi Viết Code | Tiêu Chí Nghiệm Thu Bắt Buộc |
| :--- | :---: | :--- | :--- |
| **Gói 1: Bố cục hình học Composer & Pinned Action Buttons** | **ĐỦ ĐIỀU KIỆN BẮT ĐẦU** | Không có (Đã có ảnh live Zalo PC/Web và mã layout egui). | Nút Emoji và Send ghim cố định đáy góc phải ở mọi độ cao text; thanh RTF nằm dưới TextEdit; không vỡ scroll view tin nhắn. |
| **Gói 2A: Tương tác bàn phím Composer (Mention Navigation & Enter Toggle)** | **ĐỦ ĐIỀU KIỆN BẮT ĐẦU** | Không có (Logic nội bộ client, `AppSettings` và phím tắt). | Toggle Enter trong Cài đặt có tác dụng 2 chiều với Composer; Mention điều hướng mũi tên Lên/Xuống/Enter; chặn mention khi chat 1-1. |
| **Gói 2B: Gửi Danh thiếp Chọn Nhiều (Contact Multi-Select Modal & Delivery)** | **CHƯA ĐỦ ĐIỀU KIỆN** | - Cần thiết kế component UI modal 2 cột chuẩn tỉ lệ và khống chế hạn mức 9 liên hệ.<br>- Cần cài đặt luồng gửi tuần tự từng liên hệ qua `forwardContactOA` (CmdId 11774/11827) kèm cơ chế in-flight dedup và account switch fencing. | Không gửi lặp tin khi click nhanh; hiển thị chính xác kết quả thành công/thất bại từng liên hệ; cô lập phiên tài khoản an toàn. |
| **Gói Hợp Nhất Poll (Tách riêng từ Gói 3)** | **SẴN SÀNG TRIỂN KHAI SAU KHI CÀI ĐẶT FENCING** | - Cần bổ sung struct lệnh mang ngữ cảnh phiên (`owner`, `runtime_id`, `request_id`).<br>- Cần cài đặt kiểm tra fencing 2 đầu tại Worker.<br>- Cần viết 4 bài regression tests kiểm tra fencing và readback. | Hợp nhất vào `src/group_content.rs:410`; vượt qua 4 unit test regression về fencing và readback failure; gỡ bỏ helper cũ trong `src/poll.rs`. |
| **Gói 3: Tin Nhắn Nhanh (Inline Trigger `/` & Server Contracts)** | **CHƯA ĐỦ ĐIỀU KIỆN** | - Cần hoàn thiện parser response chi tiết cho `create`/`update`/`delete`.<br>- Cần thiết kế bảng SQLite local cache mã hóa theo chuẩn fork. | Popover gợi ý inline khi gõ `/`; chọn item chỉ chèn text + preview ảnh; không tự ý gửi; cache local SQLite có mã hóa; đồng bộ với server qua `GET`. |
| **Gói 4: Quản Lý Tin Nhắn Nhanh, Migration & Gửi Nhanh Số Tài Khoản (STK)** | **CHƯA ĐỦ ĐIỀU KIỆN** | - Phụ thuộc Gói 3 (phải có command tạo Quick Message hoàn chỉnh).<br>- Cần hoàn thiện mô hình dữ liệu thẻ ngân hàng STK trong fork. | Modal sửa tin nhanh chuẩn 20 ký tự; quota guard; STK popover + modal thêm tài khoản có dropdown ngân hàng (BIN); click thẻ ngân hàng gửi ngay `POST /api/transfer/card`. |
| **Gói 5: Tùy Chọn Thêm `...`, Công Cụ zBusiness & Cô Lập zTask** | **CHƯA ĐỦ ĐIỀU KIỆN** | - Cần tách riêng R&D giao thức zTask.<br>- Hoãn thay nút To-Do bằng `View::Todo` rỗng. | Bỏ popup chắp vá `ToolsPanel`; tái lập menu `...` trên toolbar composer đúng 5 mục; gán nút Công cụ Left Rail sang popover zBusiness; giữ nguyên entry point To-Do hiện có. |

---

## 3. EVIDENCE LEDGER (SỔ BẰNG CHỨNG THỰC NGHIỆM ĐƯỢC PHÂN LOẠI CHẶT CHẼ)

### 3.1. Nhóm Bằng Chứng UI Trực Quan (UI-LIVE)
| Mã Định Danh | Môi Trường Khảo Sát | Tệp Ảnh / Vị Trí Lưu Trữ | Nội Dung Quan Sát Trực Tiếp & Đo Đạc Kỹ Thuật | Giới Hạn Của Kết Luận |
| :--- | :---: | :--- | :--- | :--- |
| **UI-LIVE-COMPOSER** | Zalo PC 26.11.12 | `zalo_pc_composer_live.png` | Thanh công cụ RTF nằm ở DƯỚI ô nhập text. Nút Emoji và Like/Gửi ghim cố định ở đáy góc phải. | Xác nhận bố cục hình học PC, không bao gồm mã nguồn backend. |
| **UI-LIVE-WEB-CHAT** | Zalo Web (`zpw_ver=695`) | `final_step_419.png` | Khung chat cơ bản, Information drawer bên phải, nút Emoji và Gửi ghim đáy. | Xác nhận layout composer Web ở trạng thái mặc định. |
| **UI-LIVE-CONTACT-MODAL** | Zalo Web (`zpw_ver=695`) | `final_step_523.png` | Modal 2 cột (`.zl-modal`): Cột trái tìm kiếm + filter chips (`Tất cả`, `Khách hàng`, `Gia đình`, `Công việc`, `Bạn bè`, `Trả lời sau`); Cột phải khay `Đã chọn: x/9` (hạn mức cứng tối đa 9 liên hệ). Checkbox góc dưới trái: `Gửi kèm số điện thoại`. Chân trang: `Hủy` + `Gửi danh thiếp`. | Hạn mức 9 là quy định UI trên phiên bản Web khảo sát, không suy diễn ra các nền tảng khác nếu chưa đo. |
| **UI-LIVE-QR-INLINE** | Zalo Web (`zpw_ver=695`) | `inline_popover_active.png`, `quick_msg_inserted_verified.png` | Gõ `/` trong composer bật popover `.popover-v3.qrs-popover`. Hint bar: *"Bấm phím ↑ ↓ để chọn, Enter để thêm vào khung soạn thảo, Ecs để ẩn."*. Bấm Enter chèn nội dung vào composer (thay thế `/keyword`), nút Like chuyển thành nút Send. **KHÔNG GỬI NGAY**. | Chỉ áp dụng cho tương tác inline `/`, không áp dụng cho các hành vi click khác nếu chưa đo. |
| **UI-LIVE-QR-QUOTA** | Zalo Web (`zpw_ver=695`) | `step_564_shot_1_zbusiness_tin_nhan_nhanh_click.png` | Modal Quản lý tin nhắn nhanh hiển thị `Tin nhắn nhanh (1/1)`. Bấm tạo thêm mẫu thứ 2 xuất hiện toast lỗi: *"Đạt giới hạn 1 tin nhắn nhanh..."*. Form sửa: phím tắt có `maxLength="20"`, bộ đếm `x/20`, textarea, nút `+ Thêm ảnh`. | **1/1 là quan sát trên tài khoản thử nghiệm Dark Baron (gói Free tiêu chuẩn)**, không phải hằng số toàn hệ thống. |
| **UI-LIVE-STK-POPOVER** | Zalo Web (`zpw_ver=695`) | `final_step_509.png` | Nút số 8 trên toolbar mở popover STK. Khi trống: hiện ảnh minh họa + nút `+ Thêm`. Modal thêm STK gồm: Dropdown chọn ngân hàng (có logo/tên viết tắt), ô số tài khoản, ô chủ tài khoản tự động viết hoa không dấu. | Xác nhận quy trình nhập liệu STK người dùng. |
| **UI-LIVE-TOOLBAR-MORE** | Zalo Web (`zpw_ver=695`) | `step_550_shot_1_toolbar_more_menu.png` | Nút `...` (vị trí thứ 9 trên toolbar) mở menu ngữ cảnh 5 mục: Tạo bình chọn, Tạo nhắc hẹn, Tạo ghi chú, Đánh dấu tin quan trọng (vàng), Đánh dấu tin khẩn cấp (đỏ). | Không chứa nút RTF hay nút đính kèm file (đây là nút bậc 1 trên toolbar). |
| **UI-LIVE-ZBUSINESS** | Zalo Web (`zpw_ver=695`) | `cong_cu_view.png`, `tool_tin_danh_dau.png` | Nút "Công cụ" trên Left Rail mở popover "Công cụ zBusiness" (Tin nhắn nhanh, Danh mục sản phẩm, Trả lời tự động, Tin đánh dấu, Tin đồng thời). Bấm "Tin đánh dấu" thay thế toàn bộ danh sách hội thoại bên trái. | Xác nhận quy hoạch chức năng zBusiness của Zalo Web. |

---

### 3.2. Nhóm Bằng Chứng Mạng Thực Tế (E1 Live Network Captures)
*Quy chuẩn E1: Chỉ ghi nhận các request thực sự đã bắt được trong phiên làm việc, có URL, method, thời điểm và phản hồi đã che dữ liệu nhạy cảm.*

| Mã Định Danh | Thời Điểm (UTC) | Phương Thức & URL Yêu Cầu | Request ID / Command ID | Dữ Liệu Phản Hồi (Sanitized Response) | Phạm Vi Kết Luận |
| :--- | :---: | :--- | :---: | :--- | :--- |
| **E1-NET-QR-LIST** | 2026-11-25 03:00 | `GET https://quickmessage.chat.zalo.me/api/quickmessage/list?zpw_ver=695&zpw_type=30&params=...` | CmdId: `12025` | `{"error_code": 0, "data": {"items": [{"id": "...", "keyword": "xinchao", "message": "...", "type": 0}], "version": 1}}` | **Chỉ chứng minh phương thức GET và cấu trúc danh sách đọc về.** Không chứng minh phương thức cho create/update/delete. |
| **E1-NET-STK-LIST** | 2026-11-25 03:20 | `GET https://zimsg.chat.zalo.me/api/transfer/list?zpw_ver=695&zpw_type=30&params=...` | CmdId: `15298` | `{"error_code": 0, "data": {"data": "<base64_aes_encrypted>"}}` (Sau giải mã: `{"myBanks": [], "total": 0, "hasMore": false}`) | **Chỉ chứng minh phương thức GET đọc danh sách thẻ ngân hàng.** Không chứng minh các luồng mutation. |

---

### 3.3. Nhóm Bằng Chứng Phân Tích Mã Gói Bundle (E2 Static Reverse Engineering)
*Quy chuẩn E2: Trích xuất trực tiếp từ thân hàm, hằng số và cấu trúc điều khiển trong bundle chính thức.*

| Mã Định Danh | Tệp Nguồn Bundle | Vị Trí Ký Tự / Thân Hàm | Cấu Trúc Kỹ Thuật Trích Xuất Được |
| :--- | :--- | :--- | :--- |
| **E2-PC-QR-API** | `compact-app-pc.0cd2e653e67efaa4bf4b.js` | Char: `15,161,790` - `15,162,200` | 4 static method: `fetchQuickReplyList` (Cmd 12025, `_get`), `createQuickReplyItem` (Cmd 12029, `_get`), `updateQuickReplyItem` (Cmd 12026, `_get`), `deleteQuickReplyItem` (Cmd 12032, `_get`). Tất cả đều truyền tham số qua query `params=encodeAES(...)`. |
| **E2-PC-QR-INLINE** | `compact-app-pc.0cd2e653e67efaa4bf4b.js` | Char: `6,131,441` - `6,131,560` | Regex kiểm tra trigger inline: `(^|\s)(\/([A-Za-z0-9])+|\/$)` và `(^|[ \t\n])\/[^ \t\n\/]+`. Handler sự kiện: `TOGGLE_DISPLAY_QR_SEARCH_POPUP`, `REPLACE_QR_KEYWORD_TO_CHAT_INPUT`. |
| **E2-PC-QR-REPLACE** | `compact-app-pc.0cd2e653e67efaa4bf4b.js` | Char: `6,720,264` | `replaceText`: Thay thế chuỗi `/keyword` bằng nội dung mẫu; nếu có ảnh (`b.type === 0`), gọi `_uploadDragPhoto` với `{ onlyPreview: true }` đưa vào khay xem trước, không gửi ngay. |
| **E2-PC-STK-SUITE** | `compact-app-pc.0cd2e653e67efaa4bf4b.js` & `main-startup.41abf8e6af2a55551e3b.js` | Char: `15,171,516` (compact-app), Char: `2,034,667` - `2,046,419` (main-startup) | 1. `getListBank`: `GET /api/transfer/conf?` (Cmd 15309, không mã hóa params).<br>2. `getListBankCard`: `GET /api/transfer/list` (Cmd 15298, params AES `{page, limit}`).<br>3. `createBankAccount`: `POST /api/transfer/create` (Cmd 15306, params AES `{bin, bank_number, holder_name, language}`).<br>4. `updateBankAccount`: `POST /api/transfer/update` (Cmd 15307, params AES `{account_id, bin, bank_number, holder_name, language}`).<br>5. `deleteBankAccount`: `POST /api/transfer/delete` (Cmd 15308, params AES `{account_id, is_default, language}`). Chặn xóa thẻ default nếu `size > 1`.<br>6. `submitBankCardInfo`: `POST /api/transfer/card` (Cmd 15220, params AES `{binBank, numAccBank, nameAccBank, cliMsgId, tsMsg, destUid, destType}`).<br>Regex: Tài khoản `A6 = /^[A-Za-z0-9]{1,19}$/`, Chủ thẻ `D6 = /^(?=.{5,50}$)[A-Za-z0-9]+(?: [A-Za-z0-9]+)*$/`. |
| **E2-PC-CONTACT** | `compact-app-pc.0cd2e653e67efaa4bf4b.js` | Char: `15,092,040` (`forwardContactOA`) | Phân nhánh: 1-1 gửi `POST /api/message/contact` (Cmd 11774); Nhóm gửi `POST /api/group/contact` (Cmd 11827). Params: `toId`/`grid`, `visibility`, `qrCodeUrl`, `oaShortLink`, `clientId`, `contactId`/`gContactId`. |
| **E2-PC-POLL** | `compact-app-pc.0cd2e653e67efaa4bf4b.js` | Char: `1,336,095` & `15,121,944` | Modal build wire object: `question, options, allow_multi_choices, allow_add_new_option, is_anonymous, poll_type: 0, expired_time: 0, pinAct`. Gửi `POST /api/poll/create` (Cmd 12080). |
| **E2-PC-PRIVACY** | `compact-app-pc.0cd2e653e67efaa4bf4b.js` | Char: `15,047,927` | `setSettings`: `_get` đến `/api/setting/update` (Cmd 12033), params là object thuộc tính trực tiếp. `updateTFASettings`: `_get` đến `/api/update/setting/2FA` (Cmd 12066). |
| **E2-PC-PIN-VAULT** | `compact-app.js` (Main process) | Char: `1,097,124` | Local PIN: `PBKDF2-HMAC-SHA256` 210,000 vòng lặp, salt 16 bytes. Electron `safeStorage` (prefix `k:`) hoặc AES-256-GCM fallback (prefix `f:`). Phân vùng `entries.${userId}.${scope}`. |
| **E2-PC-IDB-SCHEMA** | `sync-v2-worker.081738ba2350940a6b28.js` | Char: `517` | IndexedDB table `quick_message` (`id, keyword, type, createdTime, lastModified, message, media, version`). Trường `message` và `media` mã hóa AES-CBC (`AESCBCEncryption`). |

---

### 3.4. Nhóm Bằng Chứng Đối Chiếu Mã Fork Hiện Tại (CODE)
| Mã Định Danh | Tệp Mã Nguồn Fork | Vị Trí & Trạng Thái Hiện Tại | Đánh Giá Tương Thích & Khoảng Trống |
| :--- | :--- | :--- | :--- |
| **CODE-FORK-POLL** | `src/group_content.rs:400-415` | Đã có sẵn handler `CreatePoll` gọi đúng `POST /api/poll/create` (Cmd 12080) với đầy đủ tên trường chuẩn. | Đạt chuẩn contract. Cần chuyển hướng `WorkerCmd::CreatePoll` từ `tools_panel` vào đây và bổ sung fencing. |
| **CODE-FORK-POLL-LEGACY** | `src/gui/tools_panel.rs:716`, `src/poll.rs:207-230` | `tools_panel` gọi helper cũ `create_group_poll` trỏ sai route `/api/group/poll/create`. | Đường chạy lỗi thời, cần deprecate và gỡ bỏ sau khi hợp nhất. |
| **CODE-FORK-PRIVACY** | `src/privacy.rs:60-120` | Đã cài đặt đúng `GET /api/setting/update` trên `s.domains.auth` với object thuộc tính trực tiếp. | Đạt chuẩn, giữ nguyên. |
| **CODE-FORK-SESSION** | `src/session.rs:521, 572, 677` | Struct `Session` định nghĩa `pub imei: String`. Không có trường `zalo_client_id`. | Mọi request cần IMEI phải dùng `session.imei`. |
| **CODE-FORK-STORAGE** | `src/storage.rs:331` | `settings_path()` trỏ về `root().join("app_settings.json")`. Sử dụng PBKDF2 cho PIN cục bộ. | Độc lập với Zalo Desktop safeStorage, là thiết kế riêng của fork. |
| **CODE-FORK-TODO** | `src/gui.rs`, `src/gui/tools_panel.rs` | `View::Todo` hiện là stub rỗng. Nút To-Do trong `tools_panel` đang mở stub. | Chưa có dataflow zTask, cần giữ nguyên entry point hiện có, không thay bằng màn hình rỗng. |

---

## 4. CHI TIẾT CÁC HỢP ĐỒNG GIAO THỨC & AN TOÀN DỮ LIỆU (THEO ĐƯỜNG CHẠY THỰC TẾ)

Tuyệt đối không áp dụng bảng mã hóa tổng quát hay giả định mọi request đều có signkey. Dưới đây là đặc tả chi tiết theo từng route cụ thể:

### 4.1. Hợp Đồng Tin Nhắn Nhanh (Quick Messages)
- **Danh sách (List):** `GET /api/quickmessage/list?params=encodeAES({"version": v, "lang": l, "imei": session.imei})` (CmdId `12025`). Domain: `getQuickMessageDomain()`. Phản hồi thành công: `{ error_code: 0, data: { items: [...], version: ... } }`.
- **Tạo mới (Create):** `GET /api/quickmessage/create?params=encodeAES({"keyword": k, "message": m, "media": med, "type": t, "imei": session.imei})` (CmdId `12029`). Phản hồi: `{ error_code: 0, data: { item: { ... }, version: ... } }`.
- **Cập nhật (Update):** `GET /api/quickmessage/update?params=encodeAES({"itemId": id, "keyword": k, "message": m, "media": med, "type": t})` (CmdId `12026`).
- **Xóa (Delete):** `GET /api/quickmessage/delete?params=encodeAES({"itemIds": [...]})` (CmdId `12032`).
- **Cấu hình Quota Động:** Quota không phải hằng số cố định. Client Zalo PC đọc cấu hình động qua `getQuotaQuickMessage(account)` dựa trên gói dịch vụ. Hạn mức `1/1` ghi nhận trên UI là của tài khoản thử nghiệm Dark Baron (gói Free).
- **Lưu trữ Cục bộ Client:**
  * Zalo PC/Web: Bảng IndexedDB `quick_message`, các trường nhạy cảm `message` và `media` mã hóa AES-CBC.
  * Fork ctdozlite: Thiết kế bảng SQLite cục bộ `quick_messages`. Mã hóa local cache là kiến trúc riêng của fork (dùng khóa dẫn xuất từ session), không phải yêu cầu bắt buộc từ server Zalo.

---

### 4.2. Hợp Đồng Gửi Nhanh Số Tài Khoản (STK / Bank Cards)
- **Lấy Danh sách Cấu hình Ngân hàng (Conf):**
  `GET /api/transfer/conf?<common_params>` (CmdId `15309`). Domain: `getZInstantMsgDomain()`.
  *Phương thức GET thuần túy, KHÔNG CÓ tham số `params=` mã hóa.*
  Phản hồi: `{ error_code: 0, data: { banks: [ { bin, logo, name, name_eng, short_name, search_key_word } ] } }`.
- **Lấy Danh sách Tài khoản Đã Lưu (List):**
  `GET /api/transfer/list?<common_params>&params=encodeAES({"page": p, "limit": l})` (CmdId `15298`).
  Phản hồi có trường `data.data` được mã hóa AES, client giải mã bằng session secret key: `{ myBanks: [...], total, hasMore }`.
- **Tạo Tài khoản Ngân hàng (Create):**
  `POST /api/transfer/create?<common_params>` (CmdId `15306`). Body: `params=encodeAES({"bin": b, "bank_number": n, "holder_name": h, "language": lang})`.
- **Cập nhật Tài khoản (Update):**
  `POST /api/transfer/update?<common_params>` (CmdId `15307`). Body: `params=encodeAES({"account_id": id, "bin": b, "bank_number": n, "holder_name": h, "language": lang})`.
- **Xóa Tài khoản (Delete):**
  `POST /api/transfer/delete?<common_params>` (CmdId `15308`). Body: `params=encodeAES({"account_id": id, "is_default": d, "language": lang})`. Server trả lỗi `STR_BANK_ACCOUNT_CANNOT_DELETE_DEFAULT` nếu cố xóa tài khoản mặc định khi còn nhiều hơn 1 tài khoản.
- **Gửi Thẻ Ngân hàng vào Cuộc trò chuyện (Card):**
  `POST /api/transfer/card?<common_params>` (CmdId `15220`).
  Body: `params=encodeAES({"binBank": b, "numAccBank": n, "nameAccBank": h, "cliMsgId": cid, "tsMsg": ts, "destUid": uid, "destType": t})`.
  * `destType = 1` cho nhóm (`destUid` bỏ ký tự `'g'` ở đầu); `destType = 0` cho 1-1.

---

### 4.3. Hợp Đồng Gửi Danh Thiếp (Contact Card)
- **Chat 1-1:** `POST /api/message/contact` (CmdId `11774`). Domain: `chatDomain`.
  Params AES qua `buildParams`: `{ toId, clientId, contactId (hoặc gContactId nếu là OA), qrCodeUrl, oaShortLink, ... }`.
- **Chat Nhóm:** `POST /api/group/contact` (CmdId `11827`). Domain: `groupDomain`.
  Params AES: `{ grid, visibility, clientId, contactId (hoặc gContactId), qrCodeUrl, oaShortLink, ... }`.
- **Ranh giới:** Không có trường `userId`, `name`, `cliMsgId` ở root payload. Gửi đa liên hệ được thực hiện bằng cách dispatch tuần tự với `clientId` riêng biệt cho từng người nhận.

---

## 5. PHƯƠNG ÁN HỢP NHẤT POLL: FENCING VÀ PHÂN TÁCH OUTCOME

### 5.1. Tách Bạch Outcome Mutation vs Readback Failure
1. **Pha 1 (Mutation Outcome):** Khi server phản hồi request `POST /api/poll/create` (CmdId `12080`) trả về mã `poll_id` (hoặc `error_code == 0`), client xác nhận cuộc bình chọn đã **TẠO THÀNH CÔNG**. Ghi nhận `poll_id` vào state.
2. **Pha 2 (Readback):** Các lệnh gọi tiếp theo như lấy chi tiết cuộc bình chọn (`detail`) hoặc tải lại danh sách (`Action::ListPolls`) nếu gặp lỗi mạng/timeout, UI chỉ hiển thị thông báo **"Lỗi tải lại danh sách bình chọn"**, **TUYỆT ĐỐI KHÔNG báo "Tạo bình chọn thất bại"** làm người dùng hiểu lầm.
3. **Kết quả Chưa Rõ (Ambiguous / Unknown Outcome):** Nếu chính request `POST /api/poll/create` bị đứt kết nối hoặc timeout trước khi nhận response, trạng thái được đánh dấu là `UNKNOWN`. Client **tuyệt đối không tự động retry tạo mới** để tránh trùng lặp poll trên nhóm chat. Khi người dùng bấm thử lại, client phải đọc lại danh sách poll của nhóm để đối chiếu trước.

### 5.2. Rào Chắn Phiên Đầu-Cuối (End-to-End Session Fencing)
1. **Lệnh phát từ UI (`WorkerCmd::CreatePoll`):** Bắt buộc mang đầy đủ ngữ cảnh phiên phát lệnh:
   ```rust
   WorkerCmd::CreatePoll {
       owner: String,       // UIN tài khoản đang active tại thời điểm click
       runtime_id: usize,   // ID phiên làm việc (runtime session)
       request_id: u64,     // Mã định danh duy nhất của lượt click
       thread_id: String,
       question: String,
       options: Vec<String>,
       multi: bool,
       anonymous: bool,
       allow_add: bool,
   }
   ```
2. **Worker Pre-flight Check:** Trước khi serialize và gửi I/O ra mạng, Worker kiểm tra:
   `s.runtime_id() == cmd.runtime_id && s.account_key() == cmd.owner`. Nếu tài khoản đã logout hoặc switch, hủy bỏ lệnh ngay lập tức, không gửi bất kỳ byte nào ra mạng.
3. **Worker Post-flight Check (Publication Fencing):** Sau khi nhận response từ server và trước khi phát `GuiEvent` về UI, Worker kiểm tra lại một lần nữa `s.runtime_id() == cmd.runtime_id`.
4. **Scoped Completion & Error:** Mọi payload hoàn thành (`GuiEvent::GroupPollsLoaded`), trạng thái (`StatusUpdate`) và thông báo lỗi đều phải mang `runtime_id` và `request_id` tương ứng để UI chỉ render nếu phiên làm việc hiện tại vẫn trùng khớp.

### 5.3. Hợp Nhất Mã Nguồn
- Chuyển đổi `src/gui/worker.rs:7062` sang gọi trực tiếp `crate::group_content::execute(&s, &thread_id, &Action::CreatePoll { ... })`.
- Deprecate và gỡ bỏ `crate::poll::create_group_poll` trong `src/poll.rs` sau khi xác nhận không còn caller nào khác.

---

## 6. KIẾN TRÚC DI CƯ DỮ LIỆU AN TOÀN (SAFE MIGRATION ARCHITECTURE)

Khôi phục toàn bộ thiết kế chi tiết phục vụ di cư Tin nhắn nhanh từ local store cũ lên máy chủ Zalo:

```
[Local Store]
      |
      v (Chỉ đọc GET danh sách server để đối chiếu)
[Reconcile Server vs Local List]
      |
      +---> So khớp: keyword + normalized(message) + media + type
      |        |
      |        +---> Trùng khớp hoàn toàn? ---> Lưu server_item_id, đánh dấu SYNCHRONIZED
      |        |
      |        +---> Keyword trùng nhưng message/type khác? ---> Đánh dấu CONFLICT_UNKNOWN
      |
      v
[Hiển thị Modal So Sánh & Hộp Kiểm Chọn Lọc]
      |
   (Người dùng chủ động bấm "Xác nhận nhập")
      |
      v
[Lặp từng Item được chọn] ---> [Ghi Checkpoint (account_uin, item_id, PENDING)]
      |                                      |
      v                                      v
[WorkerCmd::CreateQuickMessage] ---> [Cập nhật Checkpoint: SUCCESS / FAILED]
      |
[Mất kết nối / Response bị mất?] ---> [ĐỌC LẠI GET Server List đối chiếu trước khi retry]
      |
[Bảo tồn Local Data]: Không xóa bất kỳ dữ liệu local nào nếu quá trình chưa hoàn tất 100% có xác nhận.
```

### Các Nguyên Tắc Bắt Buộc:
1. **Đối chiếu Nội dung Chuẩn hóa (Content-Aware Reconcile):** `keyword` chỉ là khóa tìm kiếm ứng viên. Để xác nhận cùng dữ liệu, client phải chuẩn hóa và so khớp: `keyword` + `message` + `media` + `type`. Nếu trùng khớp, lưu `server_item_id`. Nếu keyword trùng nhưng nội dung khác, giữ trạng thái `CONFLICT_UNKNOWN` để người dùng tự quyết định, **tuyệt đối không tự động ghi đè hoặc tự retry tạo**.
2. **Checkpointing theo `(account_uin, item_id, status)`:** Lưu tiến trình di cư vào cấu hình người dùng. Nếu xảy ra tắt ứng dụng đột ngột hoặc chuyển đổi tài khoản (account switch), khi mở lại chỉ tiếp tục xử lý các item của đúng tài khoản đang active.
3. **Xử lý Thành công Một phần (Partial Success):** Nếu người dùng chọn 5 mẫu, 3 mẫu tạo thành công và 2 mẫu bị lỗi (ví dụ do quá hạn mức), hệ thống ghi nhận thành công 3 mẫu và báo lỗi cụ thể cho 2 mẫu còn lại, không rollback hay hủy bỏ 3 mẫu đã tạo.
4. **Chống trùng lặp khi mất Response mạng (Idempotency Reconcile):** Nếu client gửi request tạo nhưng mạng bị ngắt trước khi nhận được response ACK: client **bắt buộc phải đọc lại danh sách từ server (`fetchQuickReplyList`)** và so khớp nội dung chuẩn hóa trước khi quyết định gửi lại.
5. **Bảo tồn Dữ liệu Cục bộ (Local Retention):** Không bao giờ tự động xóa dữ liệu local khi kết quả di cư chưa rõ ràng. Cung cấp tùy chọn cho người dùng: *"Giữ lại bản sao lưu cục bộ"* hoặc *"Chỉ xóa sau khi xác nhận đối chiếu thành công"*.
6. **Tiêu chí Nghiệm thu:** **Không thực hiện bất kỳ upload hay mutation nào trước khi người dùng xác nhận rõ ràng.** Cho phép client tự động gửi request đọc (`GET /api/quickmessage/list`) để phục vụ việc hiển thị giao diện đối chiếu.

---

## 7. MA TRẬN 4 CẤP ĐỘ KIỂM CHỨNG & DANH MỤC PENDING

### 7.1. Candidate Kiểm Tra Hiện Tại & Fingerprint Xác Thực
- **Git HEAD Commit:** `50c6f3eebc045306b574601d966f5cbbf8880445`
- **Tracked diff SHA-256 (`git diff HEAD`):** `82af510618b96acb1375bfc33ac355aeb4a4a42130fc1a303e9511a9c5937b56` (Mã nguồn `src/` hoàn toàn sạch, chỉ có tệp nghiên cứu được chỉnh sửa).
- **Working tree status SHA-256 (`git status --porcelain`):** `0efab31ad1cd786e35089192db289049037b2f42f7680617884345bbcf749f48`.

---

### 7.2. Ma Trận Đánh Giá Kiểm Chứng Trung Thực
*Chỉ đánh dấu PASS cho các bài test thực sự đã chạy và vượt qua trên candidate.*

| Hạng Mục Tính Năng | Cấp 1: Chưa R&D | Cấp 2: Đã Đối Chiếu E2 Bundle | Cấp 3: Test Fork (Model/Unit Test) | Cấp 4: Đã Nghiệm Thu UI-LIVE / E1 Server | Chi Tiết Trạng Thái & Bằng Chứng Thực Nghiệm |
| :--- | :---: | :---: | :---: | :---: | :--- |
| **Privacy Settings (`src/privacy.rs`)** | | [X] | **PASS (Cấp 3)** | [ ] | **TEST ĐÃ CHẠY & VƯỢT QUA:** `privacy::tests::display_setting_is_distinct_from_receive_permission_and_unknown_stays_unknown` (Finished in 0.03s). Khớp với E2 `compact-app-pc:15047927`. |
| **Hidden Chat PIN (`src/hidden_chat.rs`)** | | [X] | **PASS (Cấp 3)** | [ ] | **TEST ĐÃ CHẠY & VƯỢT QUA:** `hidden_chat::tests::verifier_is_memory_only_and_account_scoped` (Finished in 0.03s). Verifier hash nằm trên RAM, scoped theo phiên tài khoản. |
| **Scoping Bảng Tin Nhóm (`src/group_content.rs`)** | | [X] | **PASS (Cấp 3)** | [ ] | **TEST ĐÃ CHẠY & VƯỢT QUA:** `group_content::source_scope_tests::content_identity_and_kind_must_match_the_request_scoped_list` (Finished in 0.02s). Xác nhận nhận diện đúng `boardType == 3` và creator `99`. *(Chỉ chứng minh scoping, chưa kiểm tra request tạo poll)*. |
| **Fencing & Readback Poll (Hợp nhất Backend)** | | [X] | **Chưa code/test** | [ ] | **ĐÃ THIẾT KẾ 4 REGRESSION TESTS (CHƯA CODE):** Cần code và chạy 4 tests: `poll_create_success_but_readback_fails_preserves_created_status`, `poll_create_ambiguous_network_error_does_not_duplicate`, `poll_create_aborts_if_account_switched_before_network_dispatch`, `poll_create_drops_publication_if_logout_while_awaiting_response`. |
| **Bố cục Composer 2 Cột & Pinned Buttons** | | [X] | **Chưa code/test** | **PASS (Cấp 4)** | **UI-LIVE ĐÃ XÁC MINH:** Ảnh Zalo PC `zalo_pc_composer_live.png` và Web `final_step_419.png`. Chưa code trong fork. |
| **Tương tác Phím Mention & Enter Toggle** | | [ ] | **Chưa code/test** | **PASS (Cấp 4)** | **UI-LIVE ĐÃ XÁC MINH:** Web/PC có toggle Enter gửi tin; mention chặn trong 1-1. Cần thêm trường `enter_to_send` vào `AppSettings` fork. |
| **Gửi Danh Thiếp Chọn Nhiều (Gói 2B)** | | [X] | **Chưa code/test** | **PASS (Cấp 4)** | **UI-LIVE ĐÃ XÁC MINH:** Modal 2 cột, khay `Đã chọn: x/9`, checkbox SĐT (`final_step_523.png`). E2: `forwardContactOA` (Cmd 11774/11827). Chưa code UI 2 cột và worker tuần tự trong fork. |
| **Trigger Inline `/` Tin Nhắn Nhanh (Gói 3)** | | [X] | **Chưa code/test** | **PASS (Cấp 4)** | **UI-LIVE ĐÃ XÁC MINH:** Popover `.qrs-popover`, hint bar, phím Lên/Xuống/Enter chèn text, không gửi ngay (`final_step_564.png`). E2: Regex inline `compact-app-pc:6131441`. Chưa code trong fork. |
| **Bộ API Quick Messages (12025-12032)** | | [X] | **Chưa code/test** | **PASS (Cấp 4)** | **E1 CAPTURE ĐÃ BẮT:** Request `GET /api/quickmessage/list` (Cmd 12025). E2: 4 methods `_get` với `session.imei`. Chưa code worker trong fork. |
| **Phân hệ STK Thẻ Ngân Hàng (Gói 4)** | | [X] | **Chưa code/test** | **PASS (Cấp 4)** | **UI-LIVE ĐÃ XÁC MINH:** Popover STK, modal thêm tài khoản (`final_step_509.png`). E1: Capture `transfer/list`. E2: Trọn bộ 6 API và regex tài khoản/chủ thẻ viết hoa không dấu. Chưa code trong fork. |
| **Menu `...` Composer Toolbar (Gói 5)** | | [X] | **Chưa code/test** | **PASS (Cấp 4)** | **UI-LIVE ĐÃ XÁC MINH:** Menu 5 mục: Bình chọn, Nhắc hẹn, Ghi chú, Quan trọng, Khẩn cấp (`step_550_shot_1_toolbar_more_menu.png`). Chưa code trong fork. |
| **Công cụ zBusiness Left Rail (Gói 5)** | | [X] | **Chưa code/test** | **PASS (Cấp 4)** | **UI-LIVE ĐÃ XÁC MINH:** Popover 5 tiện ích; Tin đánh dấu thay thế danh sách hội thoại bên trái (`cong_cu_view.png`). Chưa code trong fork. |
| **Ghim Cuộc Bình Chọn vào Bảng Tin Nhóm** | [X] | | | | **AMBIGUOUS (CHƯA ĐỦ BẰNG CHỨNG):** Modal gọi `checkPinTopic` rồi gọi `pinBoardItem(groupId, pollId, POLL_TOPIC)`. Chưa có network capture chứng minh endpoint và request schema cụ thể. Đang giữ ở mức Pending R&D. |
| **Đồng Bộ Máy Chủ Cho zTask (To-Do)** | [X] | | | | **CHƯA ĐỦ BẰNG CHỨNG:** Chưa có bằng chứng E2 về endpoint và wire schema của zTask. Tách riêng thành track R&D độc lập. |
| **Khóa Ứng Dụng (App Lock) Trên Web** | [X] | | | | **AMBIGUOUS:** Không quan sát thấy tính năng App Lock trong phiên thử nghiệm Web zpw_ver=695 (Dark Baron). Không tìm thấy marker trong worker bundles. Giữ trạng thái Pending. |

---

## 8. KẾT LUẬN QUẢN TRỊ & HƯỚNG DẪN BÀN GIAO CHO AI TIẾP THEO

1. **Về Khởi Động Triển Khai:**
   - **Gói 1** (Bố cục Composer 2 cột) và **Gói 2A** (Enter toggle & Mention navigation) là 2 gói nội bộ client, đầy đủ chứng cứ và không có dependency mạng mở -> **ĐỦ ĐIỀU KIỆN VIẾT CODE NGAY**.
   - **Gói Hợp Nhất Poll** có thể triển khai độc lập sau khi bổ sung 4 regression tests về fencing và readback failure vào test suite của fork.
2. **Về Các Gói Còn Mở Dependency:**
   - **Gói 2B:** Giữ nguyên trạng thái chờ thiết kế UI 2 cột và luồng worker dispatch tuần tự có guard in-flight.
   - **Gói 3 & 4:** Giữ nguyên trạng thái chờ hoàn thiện parser response và SQLite cache mã hóa trước khi viết worker.
   - **Gói 5:** Không thay thế nút To-Do bằng `View::Todo` rỗng. Giữ nguyên entry point công việc hiện tại.
3. **Về Chuẩn Mực Bằng Chứng:**
   - Báo cáo này là tài liệu đối chiếu thực nghiệm kỹ thuật, không phải là tuyên ngôn tự phong. Mọi công việc sửa mã sau này đều phải tuân theo `AGENTS.md`, các tiêu chuẩn tại `docs/standards/`, và xác thực lại trên đúng candidate kiểm thử trước khi commit/push.
