# BÁO CÁO SO SÁNH TOÀN DIỆN UI/UX PHẦN COMPOSER: FORK VS ZALO WEB VS ZALO PC
**Tài liệu đối chiếu thực nghiệm, bóc tách cấu trúc DOM/CSS và kế hoạch tái thiết giao diện soạn thảo**

---

## 1. TỔNG QUAN ĐỐI CHỨNG TOÀN DIỆN PHẦN COMPOSER

Phần soạn thảo (**Composer**) là khu vực tương tác cốt lõi nhất của ứng dụng nhắn tin. Dưới đây là bảng so sánh trực tiếp giữa **Fork `ctdozlite`**, **Zalo Web (đang đăng nhập)** và **Zalo PC (đang chạy cổng debug 9222)**:

| Thành phần Composer | Hiện trạng Fork `ctdozlite` | Zalo Web (`chat.zalo.me`) | Zalo PC (`compact-app-pc.*.js`) | Đánh giá chênh lệch |
| :--- | :--- | :--- | :--- | :--- |
| **Bố cục khung nhập & Nút gửi/Like** | Gộp TextEdit và nút vào một dòng `ui.horizontal_wrapped`. Khi gõ nhiều dòng, **nút Emoji và Like bị căn giữa theo chiều dọc (middle-aligned)**, trôi lơ lửng ở giữa khung trống bên phải. | Input `contenteditable` chiếm toàn bộ chiều ngang. Nút Like/Send và Emoji **luôn cố định ở góc dưới bên phải** (`align-self: flex-end`, cách mép dưới 10px). | Cấu trúc flex 2 layout: `.chat-input-container__left-layout` (trái) và `.chat-input-container__right-layout` (phải). Các nút **luôn bám đáy** (`bottom: 10px; right: 12px`). | ❌ **Lỗi hình học nặng:** Fork làm nút nhảy lên giữa khi người dùng gõ tin nhắn dài hoặc paste nhiều dòng. |
| **Thanh công cụ Toolbar trên** | Dùng icon Lucide cơ bản (`SMILE`, `IMAGE`, `PAPERCLIP`, `MIC`, `TYPE`, `CLOCK`, `VOTE`). Sắp xếp chưa chuẩn tỷ lệ. | 9 icon nét đơn (Sticker, Photo, File, Namecard, Capture, Format Aa, Quick msg, Bank card, More `...`). | 9 icon nét đơn kích thước 24px bo tròn (`chat-box-toolbar-button --rounded`), khoảng cách 4px, tooltip tiếng Việt chuẩn. | ⚠️ **Chưa chuẩn icon & thứ tự:** Fork thiếu nút Gửi danh thiếp, Chụp màn hình có dropdown chọn chế độ, và nút Thẻ ngân hàng. |
| **Giao diện Rich Text Format (RTF `Aa`)** | Chỉ có 3 nút thô: Đậm (B), Nghiêng (I), Gạch chân (U) ở thanh công cụ hoặc bấm `Ctrl+Shift+X` thì toggle biến boolean nội bộ. | Thanh công cụ RTF nổi bật mở rộng ngay bên phải thanh soạn thảo. | Thanh công cụ RTF chuyên nghiệp (`.rtf-buttons-group`): In đậm, In nghiêng, Gạch chân, Gạch ngang, Cỡ chữ, Màu chữ (kèm vệt màu indicator), Xóa định dạng, Gạch đầu dòng, Đánh số, Thụt lề, Bỏ thụt lề, Undo, Redo, và nút Mở rộng (`Expand`). | ❌ **Quá sơ sài:** Fork thiếu 80% công cụ định dạng văn bản của Zalo PC. |
| **Khung chọn Emoji / Sticker** | Khung drawer thô (`set_height(248px)`). Tab Emoji **chỉ có 6 icon hardcode** (`👍, ❤️, 😆, 😮, 😢, 😠`). Tab GIF chỉ có vài file local. | Cửa sổ popover nổi gồm 3 tab `STICKER`, `EMOJI`, `GIF`, có ô tìm kiếm và phân loại nhóm đầy đủ. | Popover nổi kích thước chuẩn **`324px x 404px`**, bo góc 8px, bóng đổ 2 lớp (`.popover-v3`). Đầy đủ 155+ Zalo Emoticons kinh điển (`:)`, `:-*`, `/-heart`...), hàng trăm sticker động phân gói theo slider dưới đáy. | ❌ **Rất nghèo nàn:** Tab Emoji 6 icon gây thất vọng lớn, không thể sử dụng như Zalo thật. |
| **Gợi ý Sticker/GIF theo ngữ cảnh** | Có logic mảng `contextual_stickers` nhưng thanh popover vẽ thô, chưa có phím tắt chuẩn. | Thanh sticker nổi phía trên toolbar khi gõ từ khóa (`haha`, `ok`, `buồn`...). Hỗ trợ tag `@sticker`, `@gif`. | Thanh nổi ngang 70px sát trên toolbar. Bấm `Mũi tên lên` để focus, `Trái/Phải` duyệt sticker, `Enter` gửi ngay, `Esc` đóng. Hỗ trợ tag `@sticker <từ khóa>` và `@gif <từ khóa>`. | ⚠️ **Thiếu độ mượt & thiếu tag:** Chưa có tag `@sticker` / `@gif` inline. |
| **Menu nhắc tên `@mention`** | Popup đôi khi bị đè lên ô gõ chữ, tính toán vị trí theo pixel con trỏ chưa chuẩn. | Danh sách nổi **phía trên composer**, không bao giờ che khuất dòng chữ đang gõ. Có `@All`, avatar 28px, lọc theo tên thời gian thực. | Nằm sát mép trên composer (`bottom: 100%`). Header "Nhắc tên thành viên", `@All` đầu tiên, danh sách thành viên có badge Admin, phím `Lên/Xuống` + `Enter/Tab` chèn tag mượt mà. | ⚠️ **Cần tối ưu không gian:** Đảm bảo 100% không che chắn tầm nhìn văn bản đang soạn. |

---

## 2. CHI TIẾT 5 VẤN ĐỀ CỐT LÕI & GIẢI PHÁP TÁI THIẾT

### 2.1. Vấn đề 1: Nút gửi nhanh Emoji và Like icon luôn cố định ở góc dưới bên phải

#### Phân tích nguyên nhân lỗi ở Fork:
Trong `src/gui/workspace.rs` (dòng 5340–5650):
```rust
// Fork hiện tại: Gộp tất cả vào một horizontal_wrapped
ui.horizontal_wrapped(|ui| {
    // 1. TextEdit multiline với chiều cao co giãn editor_height (có thể lên tới 120px)
    egui::ScrollArea::vertical()...show(ui, |ui| text_edit.show(ui));
    
    // 2. Nút Emoji và Nút Gửi/Like đặt ngay sau TextEdit trong cùng luồng ngang
    let (emo_rect, emo_resp) = ui.allocate_exact_size(Vec2::new(32.0, 34.0), Sense::click());
    let (send_rect, send_resp) = ui.allocate_exact_size(Vec2::new(send_btn_w, 34.0), Sense::click());
});
```
- Khi người dùng gõ tin nhắn nhiều dòng hoặc dán văn bản dài, `editor_height` tăng từ 34px lên 80px - 140px. 
- Do `horizontal_wrapped` căn chỉnh các phần tử theo dòng (line-height), hai nút `emo_rect` và `send_rect` bị **căn giữa theo chiều dọc** (`middle-aligned`), nổi lơ lửng ở khoảng trống giữa chiều cao khung nhập, nhìn rất lệch lạc và khác xa Zalo gốc.

#### Chuẩn Zalo PC & Web:
- Trong Zalo PC (đã trích xuất từ DOM live):
  * Khung chứa `.chat-input-container` có `min-height: 42px`, `padding: 10px 12px`.
  * Ô nhập `.rich-input` nằm ở bên trái (`flex: 1 1 auto`), mở rộng chiều cao tự do theo số dòng văn bản.
  * Nhóm nút bên phải `.normal-buttons-group` (gồm icon Emoji và icon Like/Send) **luôn luôn cố định ở đáy** (`align-self: flex-end` hoặc đặt neo tại góc dưới bên phải: `right: 12px; bottom: 8px`). Dù người dùng gõ 1 dòng hay 10 dòng, vị trí của nút Like và Emoji không bao giờ thay đổi so với mép dưới của khung chat!

#### Giải pháp kỹ thuật cho Fork:
Tái cấu trúc lại khung soạn thảo bằng cách tính toán layout 2 phần riêng biệt:
1. Phân bổ vùng nhập liệu `editor_rect` chiếm `available_width - 80.0px`.
2. Dành riêng một cột cố định `80.0px` ở cạnh phải.
3. Trong cột phải này, tính toán vị trí Y của nút Emoji và nút Like/Send luôn neo vào `container_rect.bottom() - 38.0px` (bám đáy), hoàn toàn độc lập với chiều cao phình to của vùng nhập văn bản!

---

### 2.2. Vấn đề 2: Menu công cụ Toolbar & Giao diện Rich Text Format (`Aa`)

#### Chuẩn Toolbar Zalo PC (Thanh icon trên cùng, cao 36px):
Zalo PC sắp xếp đúng 9 mục chức năng theo thứ tự từ trái qua phải:
1. **Gửi Sticker** (`Sticker_24_Line`, `data-id="div_Sticker_Menu"`): Mở popover sticker/emoji/gif.
2. **Gửi hình ảnh** (`Photo_24_Line`): Mở hộp thoại chọn ảnh, cho phép kéo thả nhiều ảnh.
3. **Đính kèm File** (`Attach_24_Line`): Chọn tệp tin bất kỳ, sản phẩm.
4. **Gửi danh thiếp** (`Namecard_24_Line`): Mở danh bạ chọn bạn bè để gửi card liên hệ.
5. **Chụp màn hình** (`Screenshot-Z_24_Line` + icon mũi tên xuống `Chevron_Down_24_Filled`): 
   - Click icon chụp: Kích hoạt phím tắt `Ctrl + Alt + S` chụp màn hình.
   - Click mũi tên: Menu chọn `Chụp kèm cửa sổ Zalo` hoặc `Chụp KHÔNG kèm cửa sổ Zalo`.
6. **Định dạng văn bản** (`TextFormat_24_Line`, phím tắt `Ctrl + Shift + X`): Bật/Tắt thanh công cụ RTF.
7. **Tin nhắn nhanh** (`Quick-msg`): Mở danh sách phím tắt mẫu tin soạn sẵn.
8. **Số tài khoản ngân hàng** (`icon-outline-bank-card`): Chọn và gửi nhanh thông tin STK cá nhân.
9. **Tùy chọn thêm** (`More_24_Line`): Menu mở rộng (Gửi video, Tạo cuộc bình chọn, Tạo công việc To-Do).

#### Giao diện Rich Text Format (RTF) đầy đủ khi bật `Aa` (`Ctrl + Shift + X`):
Khi chế độ RTF được kích hoạt, một thanh công cụ phụ cao 32px xuất hiện với đầy đủ 14 công cụ chuyên nghiệp:
- 𝐁 **In đậm** (`Bold_24_Line` / `Ctrl+B`)
- 𝐼 **In nghiêng** (`Italic_24_Line` / `Ctrl+I`)
- <u>U</u> **Gạch chân** (`Underline_24_Line` / `Ctrl+U`)
- ~~S~~ **Gạch ngang chữ** (`Strikethrough_24_Line`)
- **Cỡ chữ** (`Titlecase_24_Line`): Chọn Nhỏ / Bình thường / Lớn / Rất lớn.
- **Màu chữ** (`Textcolor_24_Line`): Palette chọn màu văn bản kèm vệt màu indicator bên dưới.
- **Xóa định dạng** (`Erase_24_Line`): Bỏ tất cả style đang chọn về văn bản thuần.
- • **Gạch đầu dòng** (`Bullet-list_24_Line`)
- 1. **Đánh số thứ tự** (`Number-list_24_Line`)
- ⇥ **Thụt lề** (`IncreaseIndent_24_Line`) và ⇤ **Bỏ thụt lề** (`DecreaseIndent_24_Line`)
- ↺ **Hoàn tác** (`Undo_24_Line` / `Ctrl+Z`) và ↻ **Làm lại** (`Redo_24_Line` / `Ctrl+Y`)
- ⛶ **Mở rộng** (`Expand_24_Line`): Mở bung ô soạn thảo toàn màn hình để viết văn bản dài/email.

---

### 2.3. Vấn đề 3: Khung chọn Emoji / Sticker (`.popover-v3` kích thước chuẩn 324x404px)

#### Hiện trạng tệ hại của Fork:
- Tab Emoji chỉ có đúng 6 icon cứng: `👍, ❤️, 😆, 😮, 😢, 😠`.
- Không có phân loại nhóm, không có tìm kiếm, không có bảng biểu tượng cảm xúc Zalo kinh điển.

#### Chuẩn Zalo PC & Zalo Web (`.popover-v3`):
- **Kích thước:** Chiều rộng cố định `324px`, chiều cao `404px`, bo góc `8px`, bóng đổ mềm 2 tầng.
- **Cấu trúc 3 tầng rõ rệt:**

```
┌──────────────────────────────────────────────┐
│  [ STICKER ]       [ EMOJI ]       [ GIF ]   │  <- Tầng 1: 3 Tab chính (in hoa, viền xanh dưới tab active)
├──────────────────────────────────────────────┤
│  🔍  Tìm kiếm sticker / emoji...             │  <- Tầng 2: Ô tìm kiếm bo tròn 16px
├──────────────────────────────────────────────┤
│                                              │
│   (Vùng hiển thị nội dung cuộn mượt mà)      │
│   - Nếu Tab STICKER: Lưới nhãn dán động      │  <- Tầng 3: Grid nội dung cuộn
│   - Nếu Tab EMOJI: 155+ Emoticon kinh điển   │
│     của Zalo + Danh mục Unicode Emoji        │
│   - Nếu Tab GIF: Lưới ảnh động GIF           │
│                                              │
├──────────────────────────────────────────────┤
│ [🕒] [🐱 Ami] [🐻 Zookiz] [🐶 Toto] [🏪 Kho] │  <- Tầng 4: Thanh Pack Slider dưới đáy (chỉ có ở Sticker)
└──────────────────────────────────────────────┘
```

#### Chi tiết Tab EMOJI chuẩn Zalo:
1. **Phần 1: Zalo Classic Emoticons (155+ biểu cảm):**
   - Biểu tượng mặt cười Zalo màu vàng kinh điển: Cười mỉm `:)`, Hôn gió `:-*`, Khóc lóc `:((`, Lè lưỡi `:p`, Nháy mắt `;-)`, Say xỉn `$-)`, Cười lăn lộn `:))`...
   - Ký hiệu đồ vật: Trái tim `/-heart`, Trái tim vỡ `/-break`, Ly bia `/-beer`, Tách cafe `/-coffee`, Bông hồng `/-rose`, Bánh sinh nhật `/-cake`...
   - Khi click vào sẽ chèn ký tự text tương ứng vào văn bản, và khi gửi đi sẽ tự động render thành emoji động Zalo!
2. **Phần 2: Hệ thống Unicode Emoji tiêu chuẩn:**
   - Chia theo các danh mục chuẩn quốc tế:
     * 😄 *Mặt cười & Cảm xúc (Smileys & Emotion)*
     * 🐶 *Động vật & Thiên nhiên (Animals & Nature)*
     * 🍔 *Đồ ăn & Thức uống (Food & Drink)*
     * ⚽ *Hoạt động & Thể thao (Activities)*
     * 🚗 *Du lịch & Địa điểm (Travel & Places)*
     * 💡 *Đồ vật (Objects)*
     * 🔣 *Ký hiệu (Symbols)*
     * 🇻🇳 *Cờ các quốc gia (Flags)*

---

### 2.4. Vấn đề 4: Gợi ý Sticker / GIF khi gõ văn bản & Tag `@sticker`, `@gif`

#### 1. Gợi ý ngữ cảnh thông minh (Contextual Keyword Suggestions):
- Khi người dùng gõ các từ biểu đạt cảm xúc thông dụng trong tiếng Việt:
  * `haha`, `hihi`, `hoho`, `kaka`, `cuoi`: Hiện loạt sticker cười ngả nghiêng.
  * `ok`, `duoc`, `nhat tri`, `dong y`: Hiện sticker dơ tay like, gật đầu.
  * `cam on`, `thanks`, `cảm ơn`: Hiện sticker chắp tay, cúi đầu cảm ơn.
  * `chuc mung`, `congrats`: Hiện sticker tung hoa, pháo hoa.
  * `buon`, `khoc`, `sad`, `huhu`: Hiện sticker chấm nước mắt, tủi thân.
  * `yeu`, `love`, `thuong`: Hiện sticker ôm tim, bắn tim.
  * `bye`, `tam biet`, `chao`: Hiện sticker vẫy tay chào.
- **Vị trí hiển thị:** Thanh nằm ngang (cao 70px) xuất hiện **ngay phía trên thanh toolbar**, có hiệu ứng trượt nhẹ (fade-in / slide-up).
- **Hành vi tương tác bàn phím (Zalo PC Parity):**
  * `Mũi tên lên (ArrowUp)`: Nhảy focus từ ô nhập lên sticker đầu tiên.
  * `Mũi tên trái/phải (ArrowLeft/Right)`: Di chuyển chọn giữa các sticker gợi ý.
  * `Enter`: Gửi ngay sticker đang chọn (xóa từ khóa vừa gõ).
  * `Mũi tên xuống (ArrowDown)` hoặc `Esc`: Trả con trỏ về lại ô gõ văn bản, ẩn thanh gợi ý.

#### 2. Kích hoạt qua Tag chuyên biệt `@sticker` và `@gif`:
- Khi gõ `@sticker <từ khóa>` (ví dụ: `@sticker mèo`):
  * Một popover lưới nhỏ hiện ra ngay trên con trỏ hiển thị các sticker liên quan đến từ khóa "mèo".
  * Chọn sticker nào sẽ thay thế toàn bộ cụm `@sticker mèo` và gửi sticker đó đi.
- Khi gõ `@gif <từ khóa>` (ví dụ: `@gif clapping`):
  * Hiện danh sách ảnh GIF động từ kho tìm kiếm liên quan đến từ khóa.

---

### 2.5. Vấn đề 5: Menu nhắc tên `@mention` thông minh (Không che khuất văn bản)

#### Nguyên lý thiết kế chống che chắn (Non-obstructive Mentions UI):
- **Vấn đề thường gặp:** Trong một số client, menu nhắc tên được neo theo tọa độ con trỏ (cursor absolute position). Khi người dùng gõ đến dòng thứ 2 hoặc thứ 3, menu `@` bật xuống dưới hoặc nổi đè ngay lên dòng chữ đang gõ, khiến người dùng không thấy được mình đang gõ gì.
- **Quy chuẩn Zalo PC:**
  1. Menu `@mention` **luôn luôn neo cố định tại mép trên cùng của toàn bộ khung Composer** (`position: absolute; bottom: 100%; left: 0; width: 320px;`), đẩy ngược lên phía trên (về phía Timeline tin nhắn).
  2. Tuyệt đối không bao giờ đè vào vùng text box bên dưới.
  3. Chiều cao tối đa 260px, hỗ trợ cuộn dọc.
  4. Header hiển thị: `Nhắc tên thành viên` (chữ xám nhỏ 12px).
  5. Mục đầu tiên luôn là:
     * `@All - Nhắc cả nhóm` (icon loa phát thanh màu xanh dương `#0068ff`).
  6. Các mục tiếp theo là danh sách thành viên:
     * Avatar tròn 28px.
     * Tên hiển thị của thành viên (in đậm).
     * Tag chức vụ nếu có: `Trưởng nhóm` (badge vàng cam), `Phó nhóm` (badge xanh lá).
  7. **Lọc thời gian thực theo văn bản gõ tiếp:**
     * Gõ `@` $\to$ hiện toàn bộ thành viên.
     * Gõ `@Hu` $\to$ lọc ngay các thành viên có tên chứa `Hu` (Huy, Hùng, Hương...).
     * Nếu không có ai khớp $\to$ tự động đóng menu để không cản trở việc gõ bình thường.
  8. **Hành vi phím bấm:**
     * `Enter` hoặc `Tab`: Chọn thành viên đang highlight, chèn tag `@Tên` có nền xanh nhạt, tự động cách 1 khoảng trắng và đưa con trỏ ra sau để gõ tiếp.
     * `Backspace`: Nếu xóa ký tự `@` thì menu tự đóng tức thì.
     * `Esc`: Đóng menu nhắc tên ngay lập tức.

---

## 3. BẢN THIẾT KẾ KIẾN TRÚC MÃ NGUỒN CHO FORK (`src/gui/workspace.rs` & `stickers.rs`)

### 3.1. Sửa lại Layout hình học cố định nút Emoji / Like bám đáy
```rust
// Kiến trúc Composer mới trong src/gui/workspace.rs:
ui.vertical(|ui| {
    // 1. Dòng 1: Thanh công cụ Toolbar (36px)
    self.render_chat_toolbar(ui, &thread_id, is_group, pal);
    
    // Nếu RTF đang bật, render thêm dòng phụ RTF (30px)
    if self.composer_rtf_enabled {
        self.render_rtf_toolbar(ui, pal);
    }
    
    // 2. Dòng 2: Khung nhập văn bản chính (Container có viền bo tròn)
    let container_rect = ui.available_rect_before_wrap();
    egui::Frame::none()
        .fill(pal.bg_input)
        .stroke(Stroke::new(1.0, pal.border))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(10.0, 8.0))
        .show(ui, |ui| {
            let full_width = ui.available_width();
            let right_actions_width = 72.0; // 32px Emoji + 4px gap + 36px Send/Like
            let input_width = full_width - right_actions_width;
            
            ui.horizontal(|ui| {
                // Cột trái: TextEdit multiline mở rộng chiều cao tự do
                ui.allocate_ui_with_layout(
                    Vec2::new(input_width, ui.available_height()),
                    Layout::left_to_right(Align::Min),
                    |ui| {
                        // Render ScrollArea + TextEdit
                        self.render_multiline_text_edit(ui, input_width, pal);
                    }
                );
                
                // Cột phải: Luôn luôn căn đáy (Align::Max / align-self: flex-end)
                ui.allocate_ui_with_layout(
                    Vec2::new(right_actions_width, ui.available_height()),
                    Layout::right_to_left(Align::Max), // BÁM ĐÁY!
                    |ui| {
                        // Nút Send (hoặc Like nếu text trống)
                        self.render_send_or_like_button(ui, pal);
                        ui.add_space(4.0);
                        // Nút Quick Emoji
                        self.render_quick_emoji_button(ui, pal);
                    }
                );
            });
        });
});
```

### 3.2. Sửa lại cấu trúc Popover Sticker/Emoji (`src/gui/stickers.rs`)
Thay thế drawer tĩnh 248px bằng `egui::Window` hoặc `egui::Area` popover nổi kích thước chuẩn:
```rust
// Popover-v3 kích thước chuẩn Zalo PC: 324px x 404px
egui::Area::new(Id::new("zalo_sticker_popover_v3"))
    .fixed_pos(anchor_pos_above_composer)
    .show(ctx, |ui| {
        egui::Frame::none()
            .fill(pal.bg_chat_canvas)
            .rounding(Rounding::same(8.0))
            .stroke(Stroke::new(1.0, pal.border))
            .shadow(egui::epaint::Shadow {
                extrusion: 8.0,
                color: Color32::from_black_alpha(40),
            })
            .show(ui, |ui| {
                ui.set_width(324.0);
                ui.set_height(404.0);
                
                // Tầng 1: 3 Tab STICKER | EMOJI | GIF
                self.render_popover_top_tabs(ui, pal);
                
                // Tầng 2: Ô tìm kiếm bo tròn
                self.render_popover_search_bar(ui, pal);
                
                // Tầng 3: Grid nội dung cuộn (155+ Emoticon kinh điển hoặc Sticker grid)
                self.render_popover_content_grid(ui, pal);
                
                // Tầng 4: Slider danh mục gói sticker dưới đáy (nếu ở tab Sticker)
                if self.sticker_tab == StickerTab::Sticker {
                    self.render_popover_pack_slider(ui, pal);
                }
            });
    });
```

---

## 4. TỔNG KẾT BƯỚC ĐI TIẾP THEO

Báo cáo này làm rõ 100% nguyên nhân tại sao Composer của Fork hiện tại nhìn "lạ", "chắp vá" và không giống Zalo:
1. **Sửa dứt điểm vị trí nút Emoji / Like:** Đưa về neo bám đáy (`align-bottom`) cố định ở góc dưới bên phải, tách biệt khỏi chiều cao dòng của văn bản.
2. **Làm lại bộ chọn Emoji/Sticker:** Xóa bỏ bảng 6 emoji sơ sài, dựng cửa sổ Popover chuẩn `324px x 404px` với 155+ Emoticons Zalo kinh điển và kho Sticker phân gói.
3. **Hoàn thiện Rich Text Format (RTF):** Đầy đủ thanh công cụ 14 món khi bật `Aa` (`Ctrl+Shift+X`).
4. **Chuẩn hóa Menu `@mention`:** Neo cố định phía trên composer đẩy ngược lên Timeline, đảm bảo không bao giờ che khuất văn bản đang gõ.
5. **Thêm gợi ý tag `@sticker` và `@gif` inline.**
