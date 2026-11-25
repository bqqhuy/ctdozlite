# BÁO CÁO TOÀN DIỆN & TỔNG HỢP MASTER PARITY AUDIT
## Đối Soát Nghịch Đảo Zalo PC / Zalo Web & Kế Hoạch Tái Thiết Toàn Diện Zalo Lite

> **Mục đích tài liệu:** Bản tổng hợp duy nhất tập hợp toàn bộ các phát hiện, đối soát trực tiếp (live CDP inspection cổng 9222, giải nén ASAR, kiểm tra Zalo Web), các phản hồi của người dùng và lộ trình triển khai chi tiết. Tài liệu này là căn cứ tối cao để các AI Agent khác và lập trình viên đọc hiểu sâu sắc, phản biện, audit và tiến hành code mà không bị mất ngữ cảnh hay rơi vào các bẫy chắp vá của fork cũ.

---

## MỤC LỤC
1. **Bối Cảnh Xuất Phát & Nguyên Tắc Bất Di Bất Dịch**
2. **Bóc Mẽ Toàn Bộ Chắp Vá & Tồn Đọng Lớn (The Anti-Patterns)**
   - 2.1. Thảm họa nút "V" (To-Do / zTask) bị biến tướng thành popup "Tiện ích hội thoại" (`tools_panel.rs`)
   - 2.2. Lỗi hình học cố hữu: Nút Emoji & Like bị trôi lơ lửng giữa Composer
   - 2.3. Bẫy Fake/Stub ở thanh công cụ Composer và danh thiếp
   - 2.4. Phần Cài đặt (Settings Modal) chắp vá, thiếu vắng 80% tính năng cốt lõi
3. **Phân Định Tuyệt Đối: Local Stays Local vs Server Sync Must Hit Real Zalo API**
4. **Đối Soát & Đặc Tả Chi Tiết Thanh Công Cụ Composer (9 Nhóm Chức Năng)**
5. **Đặc Tả Chi Tiết Khung Soạn Thảo Rich Text Format (RTF `Aa`)**
6. **Đặc Tả Popover-v3 (Sticker, Emoji, GIF) & Gợi Ý Thông Minh**
7. **Đặc Tả Hệ Thống `@Mention` (Chỉ Group, Neo Trên Input)**
8. **Đặc Tả Tái Thiết Toàn Diện Cài Đặt (Settings Modal - 10 Tab)**
9. **TRUY QUÉT TRIỆT ĐỂ MỌI "CHẮP VÁ" & MA TRẬN TÁI THIẾT KẾ VỊ TRÍ (REMAPPING MATRIX)**
   - 9.1. Điều tra vết tích: 1,232 dòng `tools_panel.rs` & 58KB `tools.rs`
   - 9.2. Ma trận di dời 10 tính năng hợp lệ về đúng vị trí chuẩn Zalo PC/Web
   - 9.3. Danh mục các tính năng tự chế rác cần xóa bỏ vĩnh viễn
   - 9.4. Dọn sạch mạng lưới liên kết "ma" (`self.tools.open = true`)
10. **Lộ Trình Triển Khai Từng Bước (Implementation Roadmap & Quality Gates)**

---

## 1. BỐI CẢNH XUẤT PHÁT & NGUYÊN TẮC BẤT DI BẤT DỊCH

### 1.1. Thực Trạng Dự Án
- **Mục tiêu:** Xây dựng `ctdozlite` thành ứng dụng Desktop Windows 11 ARM64 (viết bằng Rust + `egui` + Tokio + SQLite) mô phỏng trải nghiệm người dùng, đồ họa pixel, và cơ chế giao thức gần với **Zalo PC chính thức** nhất có thể.
- **Vấn đề cốt lõi:** Các phiên bản trước (upstream fork) đã nhồi nhét rất nhiều tính năng tự chế (LAN transfer, auto reply, scheduled messages, fake poll, fake task) một cách chắp vá, gom tất cả vào một file rác `tools_panel.rs` (1,232 dòng mã), đồng thời giao diện Composer và Cài đặt sai lệch nghiêm trọng về UX/UI so với Zalo gốc.

### 1.2. Ba Nguyên Tắc Bất Di Bất Dịch (Non-Negotiables)
1. **Chuẩn Parity Tuyệt Đối (E1/E2 Parity):**
   - Mọi quyết định thiết kế phải dựa trên bằng chứng Zalo PC sống (CDP remote debugging `localhost:9222`) hoặc mã nguồn dịch ngược ASAR (`app.asar`), không được tự suy đoán (E3).
   - Tuyệt đối không tạo nút bấm "ma" (fake buttons) chỉ thay đổi biến tạm hoặc in toast notification mà không có luồng thực thi backend thực thụ (`No UI Stubs and No Dropped Payloads Invariant`).
2. **Local vs Server Sync Invariant:**
   - Cái gì Zalo PC xử lý **Local** (Mã PIN khóa màn hình, phím tắt, cấu hình thông báo, âm thanh chuông, thiết bị âm thanh mic/loa, cache cục bộ, hành vi phím Enter): Fork **phải xử lý Local** một cách chỉn chu, lưu trữ cấu hình bền vững.
   - Cái gì Zalo PC **Đồng bộ Server** (Tin nhắn nhanh `/api/quickmessage/*`, danh thiếp tài khoản ngân hàng `/api/transfer/card`, kho sticker đã mua/tải `/api/sticker/*`, quyền riêng tư máy chủ `/api/profile/*`): Fork **bắt buộc phải gọi đúng endpoint API của Zalo**, không được dùng mẹo vặt lưu JSON cục bộ rồi giả vờ đã có tính năng.
3. **Cổng Kiểm Tra Biên Dịch Chặt Chẽ:**
   - Mọi thay đổi code phải được kiểm tra qua:
     ```powershell
     & "$env:USERPROFILE\.cargo\bin\cargo.exe" check --locked
     ```
   - Đạt 0 lỗi biên dịch, 0 cảnh báo kiểu dữ liệu gãy trước khi báo cáo hoàn thành.

---

## 2. BÓC MẼ TOÀN BỘ CHẮP VÁ & TỒN ĐỌNG LỚN (THE ANTI-PATTERNS)

### 2.1. Thảm Họa Nút "V" (To-Do) Bị Biến Tướng Thành Popup "Tiện Ích Hội Thoại"
- **Hiện tượng trên ảnh thực tế (User uploaded `media_1795512960482_3e470b4f.jpg`):**
  Khi người dùng nhấn vào biểu tượng dấu tích `✓` trên thanh Navigation Rail màu xanh bên trái, màn hình bung ra một popup lơ lửng đè lên giữa cuộc trò chuyện mang tên **"Tiện ích hội thoại"**.
  Bên trong nhồi nhét 10 tab hổ lốn:
  * *Tìm lịch sử*, *Nhập lịch sử*, *Giao diện & sticker*, *Hẹn giờ gửi*, *Tự trả lời*
  * *Giao việc & Bình chọn*, *Tin nhắn nhanh*, *Thiết bị & Bảo mật*
  * *Kho Media Server & zCloud*, *Doanh nghiệp & LAN*
- **Sự thật trong mã nguồn fork:**
  Tại `src/gui/workspace.rs` dòng 1362-1395:
  ```rust
  // Navigation Icon 3: Giao việc (To-Do)
  let is_todo_nav = self.tools.open;
  ...
  draw_icon_todo(ui.painter(), nav_todo_rect.center(), 19.0, ...);
  if nav_todo_resp.on_hover_text(crate::i18n::tr("Giao việc (To-Do) & Tiện ích")).clicked() {
      self.tools.open = !self.tools.open;
  }
  ```
  Và tại `src/gui/tools_panel.rs`: Render toàn bộ `egui::Window::new(tr("Tiện ích hội thoại"))`.
- **Sự thật trên Zalo PC chuẩn:**
  Biểu tượng `✓` trên thanh điều hướng bên trái của Zalo PC là **zTask (Giao việc)** (`#nav-todo`).
  Khi bấm vào, Zalo PC chuyển đổi toàn bộ khung làm việc chính thành trang quản lý công việc zTask chuyên nghiệp với 3 bộ lọc:
  1. *Tôi giao*
  2. *Cần làm*
  3. *Theo dõi*
  Zalo PC **không hề có bất kỳ popup nào tên là "Tiện ích hội thoại"**. Đây là một sản phẩm tự chế của tác giả fork cũ nhằm giấu các tính năng chắp vá.

### 2.2. Lỗi Hình Học Cố Hữu: Nút Emoji & Like Bị Trôi Lơ Lửng Giữa Composer
- **Hiện tượng:** Khi người dùng nhập một đoạn tin nhắn dài từ 3 đến 10 dòng (hoặc bật chế độ định dạng RTF), khung nhập văn bản mở rộng chiều cao từ 34px lên 140px+. Nút mặt cười (Emoji) và nút Like (hoặc Gửi) lập tức bị đẩy lên và trôi lơ lửng ở giữa chiều cao của khung nhập.
- **Nguyên nhân cốt lõi trong code:**
  Tại `src/gui/workspace.rs:5340-5650`, tác giả fork đặt toàn bộ composer trong `ui.horizontal_wrapped` hoặc `ui.horizontal` đơn cấp mà không phân chia cột rõ ràng. Cột bên phải mặc định canh giữa (`Align::Center`), khiến các nút bấm bị căn lơ lửng giữa khoảng trống.
- **Thiết kế hình học chuẩn của Zalo PC (`.chat-box-input`):**
  Zalo PC chia container thành 2 cột độc lập sử dụng Flexbox:
  * Cột trái (`flex: 1 1 auto`): Chứa khung nhập văn bản `TextEdit`.
  * Cột phải (`width: 76px`, cố định): Chứa nhóm nút `.normal-buttons-group` với thuộc tính **`align-self: flex-end`** (hoặc `position: absolute; bottom: 10px; right: 12px;`).
  * **Hành vi bất biến:** Dù văn bản có dài 1 dòng, 10 dòng hay 100 dòng, hai nút Emoji và Like/Gửi luôn **nằm cố định ở góc dưới cùng bên phải**!

### 2.3. Bẫy Fake/Stub Ở Thanh Công Cụ Composer & Danh Thiếp
1. **Tin nhắn nhanh (Quick Messages):** Fork lưu cục bộ vào file `quick_messages.json` và mở tab trong `tools_panel.rs`. Trong khi đó, Zalo PC có endpoint REST API đồng bộ với server (`/api/quickmessage/list`, `/create`, `/update`, `/delete`).
2. **Gửi nhanh STK (Bank Account Card):** Fork tìm trong danh sách tin nhắn nhanh local có chuỗi `/stk` rồi dán văn bản thô vào khung chat. Zalo PC chính thức dùng API `sendBankCard` (`/api/transfer/card`) và gửi một thẻ tương tác ngân hàng có logo ngân hàng, số tài khoản, tên chủ thẻ và nút copy nhanh.
3. **Gửi Danh thiếp (Contact Card Modal):** Modal của fork hiện chỉ cho chọn dạng radio (chọn duy nhất 1 người). Zalo PC chính thức cho phép **chọn nhiều người cùng lúc (multi-select tối đa 9 liên hệ, hiển thị "Đã chọn: 0/9")**, danh sách được chia nhóm theo bảng chữ cái A, B, C... và hiển thị rõ số điện thoại.
4. **Tùy chọn thêm (`...`):** Menu của fork nhồi nhét "Giao việc", "Hẹn giờ gửi", "Thu âm thoại". Menu Zalo PC chỉ có đúng 5 mục: *Tạo bình chọn*, *Tạo nhắc hẹn*, *Tạo ghi chú*, *Đánh dấu tin quan trọng*, *Đánh dấu tin khẩn cấp*.

### 2.4. Phần Cài Đặt (Settings Modal) Chắp Vá & Thiếu Vắng Tính Năng
- Modal Cài đặt hiện tại của fork (`src/gui/modals.rs:760-2000`) là một popup hẹp, nhồi nhét các cài đặt linh tinh và thiếu hầu hết các tính năng Zalo PC:
  * Không có thanh VU meter kiểm tra âm lượng Micro theo thời gian thực.
  * Không có nút phát âm thanh thử nghiệm Loa (Test speaker beep/chime).
  * Không có nút chuyển đổi phím gửi tin nhắn (`Enter` gửi vs `Ctrl+Enter` gửi).
  * Không có cấu hình Khóa ứng dụng bằng mã PIN 4 số.
  * Không đồng bộ được danh sách 10 tab chuẩn của Zalo PC.

---

## 3. PHÂN ĐỊNH TUYỆT ĐỐI: LOCAL STAYS LOCAL VS SERVER SYNC MUST HIT REAL ZALO API

Để tránh việc AI tiếp theo lại "tiện tay" lưu mọi thứ vào file JSON cục bộ, bảng phân định sau đây là **quy chuẩn bắt buộc**:

| Tính năng | Bản chất Zalo PC | Cơ chế Fork phải triển khai | Endpoint / Cơ chế tương ứng |
| :--- | :---: | :--- | :--- |
| **Phím tắt (Shortcuts: `Ctrl+Shift+X`, `Ctrl+B`, `Ctrl+Alt+S`...)** | **100% LOCAL** | Đọc phím từ `egui::Context::input` toàn cục, lưu tùy chỉnh vào SQLite/config | `ShortcutManager` trong `src/gui/` |
| **Khóa màn hình bằng mã PIN (App Lock PIN)** | **100% LOCAL** | Lưu mã PIN băm bcrypt/argon2 trong `session.json` / SQLite, popup mở khóa khi idle | Không gửi server |
| **Âm thanh thông báo, Thiết bị Mic/Loa** | **100% LOCAL** | Dùng `cpal` liệt kê device, phát chuông qua sink âm thanh cục bộ, đo VU meter | Không gửi server |
| **Cấu hình gửi tin (`Enter` vs `Ctrl+Enter`)** | **100% LOCAL** | Boolean flag trong `AppConfig` cục bộ | Kiểm tra `Modifiers` khi bấm Enter |
| **155+ Biểu tượng cảm xúc (Emoticons `:)`, `/-heart`...) & Emoji Unicode** | **100% LOCAL** | Nhúng tài nguyên tĩnh (assets) hoặc vector/spritesheet nội bộ vào binary client | Render trực tiếp trên UI, không gọi API |
| **Tin nhắn nhanh (Quick Messages)** | **SERVER SYNC** | Gọi API Zalo server để lấy danh sách, thêm, sửa, xóa | `GET /api/quickmessage/list`<br>`POST /api/quickmessage/create`<br>`POST /api/quickmessage/delete` |
| **Gửi thẻ tài khoản ngân hàng (Bank Card)** | **SERVER SYNC** | Gửi payload cấu trúc thẻ qua endpoint Zalo | `POST /api/transfer/card` (`sendBankCard`) |
| **Kho Sticker (Danh mục pack, tải pack mới)** | **SERVER SYNC (HYBRID)** | Đồng bộ danh mục pack đã sở hữu từ server, tải spritesheet từ CDN và cache SQLite | `GET /api/sticker/get-category-list`<br>CDN Zalo download sprite |
| **Danh thiếp liên hệ (Contact Card)** | **SERVER SYNC** | Lấy danh bạ từ Local Store / Server, gửi tin nhắn dạng `CONTACT` (type 10) | `WorkerCmd::SendContactCard` (Command ID chuẩn Zalo) |
| **Tạo bình chọn (Poll)** | **SERVER SYNC** | Tạo poll qua API nhóm của Zalo | `POST /api/group/poll/create` |
| **Quyền riêng tư (Chặn tin nhắn, ẩn ngày sinh)** | **SERVER SYNC** | Đồng bộ trực tiếp với profile Zalo | `POST /api/profile/update-privacy` |

---

## 4. ĐỐI SOÁT & ĐẶC TẢ CHI TIẾT THANH CÔNG CỤ COMPOSER (9 NHÓM CHỨC NĂNG)

Thanh công cụ Composer nằm ngay phía trên khung nhập văn bản, bao gồm chính xác 9 nút theo thứ tự từ trái sang phải:

```
[Icon 1: Sticker] [Icon 2: Gửi Ảnh] [Icon 3: Đính Kèm File] [Icon 4: Gửi Danh Thiếp] [Icon 5: Chụp Màn Hình ▾] [Icon 6: Định Dạng Aa] [Icon 7: Tin Nhắn Nhanh ⚡] [Icon 8: Gửi Thẻ STK] [Icon 9: Thêm ...]
```

### Chi Tiết Từng Nút:
1. **Nút 1: Sticker & Emoticon (`Sticker_24_Line`)**
   - *Hành vi:* Bật/tắt Popover-v3 (kích thước 324px x 404px).
2. **Nút 2: Gửi hình ảnh (`Photo_24_Line`)**
   - *Hành vi:* Mở hộp thoại chọn tệp của Windows (`rfd::FileDialog`), lọc các định dạng ảnh (`.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`). Hỗ trợ chọn nhiều ảnh cùng lúc và kéo thả trực tiếp.
3. **Nút 3: Đính kèm tệp tin / Thư mục (`Attach_24_Line`)**
   - *Hành vi:* Khi click hiển thị menu nhỏ gồm 2 tùy chọn:
     * *Chọn tập tin* (mở file picker đa tệp).
     * *Chọn thư mục* (mở folder picker, nén zip hoặc gửi thư mục theo giao thức Zalo).
4. **Nút 4: Gửi danh thiếp (`Contact_24_Line`)**
   - *Hành vi:* Mở modal `Gửi danh thiếp` kích thước 480px x 560px.
   - *Đặc tả giao diện modal:*
     * Ô tìm kiếm trên cùng: *"Tìm kiếm liên hệ"*.
     * Danh sách hiển thị có checkbox hỗ trợ **chọn tối đa 9 liên hệ cùng lúc**. Phía dưới góc có nhãn *"Đã chọn: X/9"*.
     * Danh sách được gom nhóm theo bảng chữ cái A, B, C...
     * Mỗi item hiển thị Avatar tròn 36px, Tên danh bạ, và Số điện thoại (nếu có).
     * Nút bấm: *"Hủy"* và *"Chia sẻ"* (chỉ active khi X > 0).
5. **Nút 5: Chụp màn hình (`Screenshot_24_Line` kèm mũi tên xổ xuống `Chevron_Down`)**
   - *Hành vi:* Là một nút phân tách (split button) hoặc nút có menu phụ:
     * Click thẳng vào icon: Kích hoạt chụp màn hình ngay lập tức (phím tắt mặc định `Ctrl+Alt+S`).
     * Click vào mũi tên nhỏ: Hiện 2 tùy chọn:
       1. *Chụp kèm cửa sổ Zalo* (mặc định).
       2. *Chụp KHÔNG kèm cửa sổ Zalo* (tự động ẩn cửa sổ Zalo trước khi chụp).
6. **Nút 6: Soạn văn bản định dạng RTF (`Text_Format_24_Line` / `Aa`)**
   - *Hành vi:* Chuyển đổi qua lại giữa chế độ soạn thảo đơn giản và chế độ RTF mở rộng (Xem mục 5).
7. **Nút 7: Tin nhắn nhanh (`Quick_Message_24_Line` / Biểu tượng tia sét `⚡`)**
   - *Hành vi:* Bật popover nhỏ gọn danh sách tin nhắn nhanh đã đồng bộ từ server qua `/api/quickmessage/list`.
   - *Chi tiết hiển thị:*
     * Danh sách các mẫu tin nhắn nhanh gồm: Phím tắt (VD: `/chao`), Nội dung tóm tắt, và hình ảnh đính kèm (nếu có).
     * Click vào một mục: Dán nội dung vào khung soạn thảo hoặc gửi ngay lập tức.
     * Cuối popover có nút: *"Quản lý tin nhắn nhanh"* (chuyển đến trang cấu hình chuẩn trong Cài đặt).
8. **Nút 8: Gửi nhanh thông tin tài khoản / Thẻ ngân hàng (`Bank_Card_24_Line`)**
   - *Hành vi:* Mở danh sách các tài khoản ngân hàng đã lưu của người dùng. Khi chọn, client gửi tin nhắn cấu trúc thẻ ngân hàng `sendBankCard` (`/api/transfer/card`) hiển thị chuyên nghiệp với nút sao chép STK một chạm.
9. **Nút 9: Tùy chọn thêm (`More_Horizontal_24_Line` / `...`)**
   - *Hành vi:* Mở menu xổ xuống chứa chính xác 5 mục chuẩn của Zalo PC:
     1. *Tạo bình chọn* (mở modal tạo Poll nhóm chuẩn).
     2. *Tạo nhắc hẹn* (mở modal Reminder chuẩn).
     3. *Tạo ghi chú* (mở bảng tạo Note ghim nhóm).
     4. *Đánh dấu tin quan trọng* (tô cờ ưu tiên).
     5. *Đánh dấu tin khẩn cấp* (báo động viền đỏ).
   - **Tuyệt đối không nhồi nhét các tính năng tự chế vào menu này!**

---

## 5. ĐẶC TẢ CHI TIẾT KHUNG SOẠN THẢO RICH TEXT FORMAT (RTF `Aa`)

Khi người dùng nhấn vào nút `Aa`, khung nhập chuyển sang chế độ `--rtf-mode`:

```
+-----------------------------------------------------------------------------------+
|  [B] [I] [U] [S] | [Cỡ chữ ▾] [Màu ▾] [Xóa định dạng] | [• Danh sách] [1. Số]   |  <- Thanh 14 công cụ RTF
|  [Thụt lề trái] [Thụt lề phải] | [Hoàn tác] [Làm lại] | [Thu nhỏ / Mở rộng]       |
+-----------------------------------------------------------------------------------+
| Tiêu đề đoạn văn bản                                                              |  <- Khung nhập TextEdit
| Nội dung chi tiết có định dạng...                                                 |     (Chiều cao cố định 178px,
| (Nếu nội dung vượt quá chiều cao, thanh cuộn dọc mỏng xuất hiện tại đây)          |      có Scrollbar khi tràn)
|                                                                                   |
|                                                              [Icon Mặt cười] [Gửi]|  <- Nhóm nút CỐ ĐỊNH góc dưới
+-----------------------------------------------------------------------------------+
```

### Yêu Cầu Kỹ Thuật Chi Tiết:
1. **Chiều Cao & Thanh Cuộn (Scrollbar):**
   - Khi ở chế độ RTF, khung chứa có chiều cao ban đầu chuẩn là **178px**.
   - Khi văn bản vượt quá 160px, khung nhập kích hoạt `egui::ScrollArea::vertical()` hiển thị thanh cuộn dọc mỏng (chiều rộng 4px, màu xám nhạt, bo góc 2px) bên trong khung nhập, không làm vỡ bố cục tổng thể.
2. **Vị Trí Thanh Định Dạng:**
   - Thanh 14 công cụ định dạng **bắt buộc phải nằm ở TRÊN CÙNG** của khung nhập văn bản.
3. **Phím Tắt Định Dạng Bắt Buộc:**
   - `Ctrl + B`: In đậm (Bold).
   - `Ctrl + I`: In nghiêng (Italic).
   - `Ctrl + U`: Gạch chân (Underline).
   - `Ctrl + Z`: Hoàn tác (Undo).
   - `Ctrl + Y`: Làm lại (Redo).

---

## 6. ĐẶC TẢ POPOVER-V3 (STICKER, EMOJI, GIF) & GỢI Ý THÔNG MINH

### 6.1. Kích Thước & Cấu Trúc Khung Chọn Popover-v3
- **Kích thước:** Chiều rộng chuẩn **324px**, chiều cao chuẩn **404px**, bo góc 8px, đổ bóng 2 lớp theo chuẩn Zalo PC.
- **Thanh Tab Trên Cùng (3 Tab):** `STICKER`, `EMOJI`, `GIF`.
- **Thanh Tìm Kiếm:** Placeholder: *"Tìm kiếm sticker / emoji"*.
- **Khu Vực EMOJI (Local 100%):** Chứa đầy đủ **155+ biểu tượng Zalo cổ điển** (`:)`, `:-*`, `/-heart`, `/-coffee`, `/-rose`...) và bảng phân nhóm Emoji Unicode chuẩn.
- **Khu Vực STICKER (Hybrid):** Thanh trượt icon các bộ sticker đã tải ở chân popover. Khi click vào một bộ, hiển thị lưới sticker tương ứng.

### 6.2. Cơ Chế Gợi Ý Sticker / GIF Thông Minh
1. **Gợi ý theo từ khóa gõ văn bản:** Khi người dùng gõ từ khóa cảm xúc ("haha", "ok", "buon", "yeu"), một thanh gợi ý sticker nhỏ (chiều cao ~56px) nổi lên ngay trên thanh công cụ Composer chứa 4-6 sticker để gửi ngay.
2. **Gợi ý qua Inline Tag:**
   - `@sticker <từ khóa>`: Mở popover tìm kiếm sticker.
   - `@gif <từ khóa>`: Mở khung tìm kiếm GIF.

---

## 7. ĐẶC TẢ HỆ THỐNG `@MENTION` (CHỈ GROUP, NEO TRÊN INPUT)

1. **Điều Kiện Kích Hoạt Nghiêm Ngặt:**
   - **Chỉ kích hoạt trong Group Chat / Community:** Trong chat 1-on-1 hoặc Cloud cá nhân, ký tự `@` là văn bản thuần túy (gõ `user@gmail.com` không bao giờ hiện popup).
2. **Neo Vị Trí Tuyệt Đối Không Che Chắn Văn Bản:**
   - Popup mention phải được neo ở vị trí **`bottom: 100%` (phía trên khung nhập)**, đẩy ngược lên vùng timeline tin nhắn. Tuyệt đối không che khuất con trỏ soạn thảo!
3. **Nội Dung & Hành Vi:**
   - Dòng đầu tiên: `@All (Nhắc cả nhóm)`.
   - Các dòng tiếp theo: Thành viên nhóm (Avatar 28px, Tên hiển thị).
   - Bộ lọc thời gian thực khi gõ tiếp ký tự sau `@`.
   - Phím `Enter` hoặc `Tab` chèn `@TênThànhViên ` kèm dấu cách phía sau.

---

## 8. ĐẶC TẢ TÁI THIẾT TOÀN DIỆN CÀI ĐẶT (SETTINGS MODAL - 10 TAB)

Modal Cài đặt kích thước chuẩn **800px x 800px**, chia 2 cột:
- **Cột Trái (220px):** Danh sách 10 tab chuẩn:
  1. *Cài đặt chung* (Khởi động cùng Windows, ngôn ngữ)
  2. *Tin nhắn* (Enter để gửi vs Ctrl+Enter để gửi, gợi ý sticker, quản lý tin nhắn nhanh)
  3. *Thông báo* (Âm thanh thông báo, popup thông báo)
  4. *Cuộc gọi & Video* (Chọn Mic/Loa, thanh đo VU Meter mic dao động real-time, nút test loa)
  5. *Tiện ích*
  6. *Giao diện* (Sáng, Tối, Tự động)
  7. *Phím tắt* (Danh sách phím tắt hệ thống `Ctrl+Alt+S`, `Ctrl+Shift+X`...)
  8. *Riêng tư & Bảo mật* (Mã khóa ứng dụng PIN 4 số, thiết bị đăng nhập)
  9. *Dữ liệu & Bộ nhớ* (Quản lý dung lượng cache, sao lưu & khôi phục tin nhắn)
  10. *Thông tin phiên bản* (Phiên bản, kiểm tra cập nhật)

---

## 9. TRUY QUÉT TRIỆT ĐỂ MỌI "CHẮP VÁ" & MA TRẬN TÁI THIẾT KẾ VỊ TRÍ (REMAPPING MATRIX)

Đây là phần trọng tâm đáp ứng chỉ đạo: **Rà soát triệt để toàn bộ mã nguồn, tìm kiếm mọi mảnh chắp vá, xóa bỏ những gì không chuẩn và tái thiết kế vị trí (remap) cho các tính năng hợp lệ về đúng vị trí như Zalo PC/Web.**

### 9.1. Điều Tra Vết Tích Chắp Vá: `tools_panel.rs` (1,232 Dòng) & `tools.rs` (58KB)
Trong mã nguồn hiện tại, sự chắp vá tập trung vào 2 tệp lớn:
1. `src/gui/tools_panel.rs`: Render một popup trôi nổi tự chế `egui::Window` với 10 tab hổ lốn.
2. `src/tools.rs`: Chứa backend gom chung cả preferences, quick messages, quy tắc reply, scheduled tasks, và LAN transfer.
3. **Mạng lưới kích hoạt "ma":** Thay vì xây dựng UI đúng chỗ, fork đã cắm các lệnh gọi `self.tools.open = true` vào hàng loạt nơi:
   - Thanh Navigation Rail (nút `✓` To-Do) -> Mở tools popup!
   - Menu `...` trên Composer ("Giao việc", "Tạo bình chọn", "Hẹn giờ", "Tin nhắn nhanh") -> Mở tools popup!
   - Settings Modal ("Tìm lịch sử, nhập dữ liệu, theme...") -> Mở tools popup!

---

### 9.2. Ma Trận Tái Thiết Kế & Di Dời (Remapping Matrix)

Mọi tính năng **hợp lệ** đang bị giam cầm trong popup `tools_panel.rs` phải được **di dời và tái thiết kế về đúng vị trí chuẩn Zalo PC/Web** theo bảng ma trận sau:

| STT | Tính Năng Hợp Lệ | Vị Trí Cũ (Chắp Vá) | Vị Trí Chuẩn Zalo PC/Web (Tái Thiết Kế) | Phương Án Triển Khai Kỹ Thuật |
| :---: | :--- | :--- | :--- | :--- |
| **1** | **Giao việc (zTask / To-Do)** | Tab 5 trong popup "Tiện ích hội thoại" | **Biểu tượng `✓` trên Navigation Rail bên trái** (`#nav-todo`) | Khi click nút `✓`, chuyển toàn bộ màn hình chính sang **Trang Giao việc zTask** (3 tab: *Tôi giao*, *Cần làm*, *Theo dõi*). Không mở popup trôi nổi! |
| **2** | **Tin nhắn nhanh (Quick Messages)** | Tab 6 trong popup "Tiện ích hội thoại" (lưu JSON local) | **1. Nút tia sét `⚡` trên thanh công cụ Composer**<br>**2. Tab "Tin nhắn" trong Cài đặt (Settings)** | - Nút `⚡` mở popover nhỏ để chọn chèn nhanh.<br>- Nút "Quản lý" ở chân popover hoặc vào Cài đặt -> Tin nhắn để thêm/sửa/xóa.<br>- **Đồng bộ trực tiếp Server API** `/api/quickmessage/*`. |
| **3** | **Tìm kiếm lịch sử tin nhắn** | Tab 0 trong popup "Tiện ích hội thoại" | **1. Thanh tìm kiếm toàn cục góc trên trái (`Ctrl+K`)**<br>**2. Tìm trong hội thoại ở Header chat (`Ctrl+F`)** | - Tích hợp truy vấn SQLite `store.search_messages` vào thanh tìm kiếm chính.<br>- Khi bấm `Ctrl+F` trong chat, mở thanh tìm kiếm nội bộ hội thoại ở góc trên bên phải khung tin nhắn. |
| **4** | **Sao lưu / Khôi phục lịch sử** | Tab 1 trong popup ("Nhập lịch sử") | **Tab 9 "Dữ liệu & Bộ nhớ" trong Cài đặt** (Settings Modal) | Đặt tại mục *Sao lưu & Khôi phục* (Backup & Restore) chuẩn Zalo PC. Cho phép xuất/nhập file sao lưu SQLite/JSON có mã hóa. |
| **5** | **Giao diện & Chủ đề (Theme)** | Tab 2 trong popup ("Giao diện & sticker") | **Tab 6 "Giao diện" trong Cài đặt** (Settings Modal) | Đặt tại Cài đặt -> Giao diện: Chọn theme Sáng (Light), Tối (Dark), hoặc Đồng bộ theo hệ thống Windows. |
| **6** | **Quản lý Sticker đã tải** | Tab 2 trong popup ("Giao diện & sticker") | **Chân Popover Sticker-v3 (`.popover-v3`) & Cửa hàng Sticker** | Các gói sticker hiển thị trên thanh trượt dưới đáy Popover-v3. Bổ sung nút hình bánh răng hoặc dấu cộng để quản lý/tải thêm sticker. |
| **7** | **Thiết bị & Bảo mật** | Tab 7 trong popup ("Thiết bị & Bảo mật") | **Tab 8 "Riêng tư & Bảo mật" trong Cài đặt** (Settings Modal) | Hiển thị danh sách thiết bị đang đăng nhập, nút đăng xuất từ xa, và cài đặt mã khóa PIN 4 số của ứng dụng. |
| **8** | **Tạo bình chọn (Poll)** | Nhồi chung vào Tab 5 ("Giao việc & Bình chọn") | **Menu Tùy chọn thêm (`...`) trên Composer** | Khi bấm vào "Tạo bình chọn" trong menu `...`, mở **Modal Tạo bình chọn nhóm chuẩn** (Câu hỏi, các lựa chọn, thời hạn, ghim lên đầu nhóm). Gọi API `/api/group/poll/create`. |
| **9** | **Kho Media & Lưu trữ zCloud** | Tab 8 trong popup ("Kho Media Server & zCloud") | **1. Bảng Thông tin hội thoại (Right Sidebar)**<br>**2. Hội thoại đặc biệt "Cloud của tôi"** | Right Sidebar chia 3 tab: *Hình ảnh/Video*, *Tập tin*, *Liên kết*. "Cloud của tôi" là thread riêng biệt đồng bộ file cá nhân. |
| **10** | **Quản lý Phím tắt hệ thống** | Rải rác, không có chỗ cấu hình chuẩn | **Tab 7 "Phím tắt" trong Cài đặt** (Settings Modal) | Liệt kê toàn bộ bảng phím tắt (`Ctrl+Alt+S` chụp ảnh, `Ctrl+Shift+X` mở nhanh, `Ctrl+B/I/U` RTF, `Ctrl+F` tìm kiếm). Cho phép tùy biến và gán lại phím. |

---

### 9.3. Danh Mục Các Tính Năng Tự Chế "Rác" Cần Xóa Bỏ Vĩnh Viễn

Các tính năng sau đây là do tác giả fork cũ tự sáng tạo, không hề tồn tại trên Zalo PC / Zalo Web chính thức, gây rối loạn kiến trúc và tạo lỗ hổng:

1. **Module Doanh Nghiệp & LAN Transfer (Tab 9 trong popup):**
   - *Thực trạng:* Tác giả tự viết socket quét mạng cục bộ để truyền file ngang hàng (P2P LAN).
   - *Quyết định:* **Xóa bỏ hoàn toàn.** Zalo PC gửi file qua máy chủ CDN Zalo / zCloud (`/api/file/*`). Không hỗ trợ giao thức LAN tự chế không an toàn.
2. **Module Hẹn Giờ Gửi Tin Nhắn Cá Nhân (Tab 3 trong popup):**
   - *Thực trạng:* Cơ chế đếm giờ local để tự bắn tin nhắn. Zalo PC cá nhân không có tính năng này.
   - *Quyết định:* **Xóa bỏ khỏi giao diện người dùng chính.** Nếu người dùng yêu cầu giữ lại dưới dạng công cụ nâng cao (Power-User Feature), nó phải được ẩn vào mục Developer Tools hoặc tiện ích phụ trợ mở rộng, tuyệt đối không được xuất hiện trên thanh công cụ Composer hay menu `...`.
3. **Module Tự Trả Lời Tin Nhắn Cá Nhân Kiểu Bot Tự Chế (Tab 4 trong popup):**
   - *Thực trạng:* Tự động quét từ khóa để trả lời lại tin nhắn. Đây là tính năng của Zalo Official Account (OA) hoặc Zalo Business, không phải của Zalo PC người dùng cuối.
   - *Quyết định:* **Xóa bỏ triệt để khỏi popup chat.**

---

### 9.4. Dọn Sạch Mạng Lưới Liên Kết "Ma" (`self.tools.open = true`)

Để chuẩn bị xóa sổ hoàn toàn `src/gui/tools_panel.rs`, phải tiến hành refactor và gỡ bỏ toàn bộ các vị trí đang gọi biến `self.tools`:

1. **`src/gui/workspace.rs`:**
   - Dòng 1362-1395: Đổi `let is_todo_nav = self.tools.open;` thành `let is_todo_nav = self.current_view == View::Todo;`. Khi bấm nút `✓`, chuyển `self.current_view = View::Todo;`.
   - Dòng 5222, 5267, 5276, 5287, 5292: Gỡ bỏ việc gán `self.tools.open = true`. Thay thế bằng mở đúng modal chuẩn (VD: modal Tạo bình chọn `self.poll_modal_open = true`).
   - Dòng 5696, 5701, 5718: Gỡ bỏ menu nhảy vào `self.tools.tab = 3` hay `tab = 6`. Thay bằng popover tin nhắn nhanh chuẩn.
2. **`src/gui/modals.rs`:**
   - Dòng 1608, 2025: Xóa bỏ nút bấm mở `self.tools.open = true`.
3. **`src/gui.rs`:**
   - Dòng 1422, 2117: Xóa trường `tools: tools_panel::ToolsPanel` khỏi `struct ZaloApp`.
   - Dòng 5100-5290: Xóa bỏ khối xử lý sự kiện `tools_panel::ToolsResult`.
   - Dòng 55: Xóa khai báo `mod tools_panel;`.
4. **Xóa tệp:** Xóa vĩnh viễn tệp `src/gui/tools_panel.rs`. Tinh gọn `src/tools.rs` chỉ giữ lại backend cho Search và Backup trong store.

---

## 10. LỘ TRÌNH TRIỂN KHAI TỪNG BƯỚC (IMPLEMENTATION ROADMAP & QUALITY GATES)

### Giai Đoạn 1: Sửa Lỗi Hình Học Composer & Pinned Buttons (Ưu Tiên 1)
- **Tập tin:** `src/gui/workspace.rs`
- **Mục tiêu:**
  * Chia container soạn thảo thành 2 cột Flexbox rõ ràng.
  * Cột phải (rộng 76px) chứa Emoji icon và Like/Send button **luôn được ghim chặt ở góc dưới cùng bên phải (`Align::Max`)** bất kể khung nhập mở rộng bao nhiêu dòng.
- **Tiêu chí nghiệm thu:** Gõ 10 dòng chữ, hai nút vẫn nằm ngay ngắn ở góc dưới cùng, không trôi lơ lửng.

### Giai Đoạn 2: Tái Thiết Khung Soạn Thảo RTF (`Aa`) & Phím Tắt
- **Tập tin:** `src/gui/workspace.rs`
- **Mục tiêu:**
  * Thiết lập chiều cao chuẩn 178px khi bật RTF.
  * Bổ sung thanh cuộn dọc nội bộ (ScrollArea) khi văn bản tràn.
  * Đặt thanh 14 công cụ định dạng lên trên cùng.
  * Bắt các phím tắt `Ctrl+B`, `Ctrl+I`, `Ctrl+U`, `Ctrl+Z`, `Ctrl+Y`.
- **Tiêu chí nghiệm thu:** Đạt đối soát trực quan với cửa sổ Zalo PC live (screenshot `zalo_pc_composer_live.png`).

### Giai Đoạn 3: Popover-v3 (Sticker, 155+ Emoticons, GIF) & Mention An Toàn
- **Tập tin:** `src/gui/stickers.rs`, `src/gui/mention_composer.rs`
- **Mục tiêu:**
  * Dựng Popover-v3 kích thước 324px x 404px với 3 tab: STICKER, EMOJI, GIF.
  * Tích hợp 155+ biểu tượng Zalo emoticons cục bộ.
  * Khóa tính năng `@mention` chỉ hoạt động trong Group/Community và neo vị trí phía trên khung nhập.
- **Tiêu chí nghiệm thu:** Gõ email trong chat 1-1 không bung popup; trong group popup hiện phía trên, không che chữ.

### Giai Đoạn 4: Đồng Bộ Server Tin Nhắn Nhanh & Modal Danh Thiếp Multi-Select
- **Tập tin:** `src/chat.rs`, `src/gui/worker.rs`, `src/gui/workspace.rs`
- **Mục tiêu:**
  * Triển khai endpoint `/api/quickmessage/list` lấy tin nhắn nhanh thật từ server.
  * Nâng cấp modal Gửi danh thiếp thành dạng checkbox multi-select (tối đa 9 người, gom nhóm A-Z).
  * Chuẩn hóa menu Tùy chọn thêm (`...`) đúng 5 mục chuẩn.

### Giai Đoạn 5: Triệt Tiêu Chắp Vá `tools_panel.rs` & Tái Thiết Modal Cài Đặt 800x800px
- **Tập tin:** `src/gui/modals.rs`, xóa `src/gui/tools_panel.rs`, tái cấu trúc `workspace.rs`
- **Mục tiêu:**
  * Thực hiện toàn bộ Ma trận di dời (Remapping Matrix) mục 9.2: Chuyển To-Do về nút `✓`, tin nhắn nhanh về nút `⚡`, tìm kiếm về `Ctrl+F` / `Ctrl+K`.
  * Xóa bỏ hoàn toàn `tools_panel.rs` và các tính năng rác LAN/Auto-reply.
  * Dựng Settings Modal 10 tab kích thước 800x800px với thanh đo VU meter âm thanh micro và nút phát thử nghiệm loa.

---

> **LỜI DẶN DÀNH CHO AI TIẾP THEO:**
> Bạn có trong tay ảnh chụp màn hình Zalo PC thật tại `C:\Users\bqqhu\.gemini\antigravity\brain\85bfbcb4-520a-40bc-8bc7-9f7bad6db47d\zalo_pc_composer_live.png` và tiến trình Zalo PC đang chạy cổng debug 9222. Hãy bám sát từng pixel và từng quy tắc phân định Local/Server trong tài liệu này để triển khai. Trước khi kết thúc bất kỳ bước nào, hãy chạy lệnh `& "$env:USERPROFILE\.cargo\bin\cargo.exe" check --locked` để đảm bảo hệ thống luôn biên dịch hoàn hảo.
