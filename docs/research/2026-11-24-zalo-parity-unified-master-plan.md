# Kế hoạch Hành động Tổng thể Hợp nhất Zalo Parity (ctdozlite)
**Tài liệu:** `docs/research/2026-11-24-zalo-parity-unified-master-plan.md`\
**Thời điểm lập:** 24/11/2026 (Cập nhật toàn diện sau phân tích Composer E1)\
**Baseline commit:** `c9a274f` (`feat(sync): remediate audit rounds 1-4 for sync coordinator, config and backup`)\
**Mục tiêu:** Bổ sung toàn bộ các tính năng, luồng giao tiếp server và cải tiến UX còn thiếu so với Zalo gốc (PC & Web), đồng thời sửa dứt điểm các lỗi phát hiện qua kiểm thử thực tế. Mọi thành phần đều phải được đấu nối khép kín end-to-end (`UI -> WorkerCmd -> Network/DB -> GuiEvent -> UI Render`), tuyệt đối không để code mồ côi hoặc UI stub.

---

## I. TỔNG QUAN 6 KHỐI CÔNG VIỆC (PHASES) HỢP NHẤT

```text
+----------------------------------------------------------------------------------------------------+
| PHASE 1 (P0): TRIỆT TIÊU LỖI CHUÔNG VOIP DAI DẲNG & PREFLIGHT                                     |
| • actor_is_self chuẩn hóa • ReceiveCall/MissCall ngắt chuông CPAL • Watchdog 65s • Preflight Mic/Loa|
+----------------------------------------------------------------------------------------------------+
| PHASE 2 (P0): TÁI THIẾT KẾ TOÀN DIỆN COMPOSER & ĐÍNH KÈM TỆP (E1 PARITY)                           |
| • Toolbar 9 nút chuẩn & popup menu (...) • Thẻ tệp 72x72px bo góc 8px • Xóa chặn metadata file    |
| • Gửi song song file + caption text • Nút Thích -> Gửi xanh • Gợi ý sticker nổi • Modal danh thiếp|
+----------------------------------------------------------------------------------------------------+
| PHASE 3 (P0): TƯƠNG TÁC TIN NHẮN CHUẨN UX ZALO & VÒNG ĐỜI TỆP LOCAL                               |
| • Xóa trễ hover 150ms • MQA nổi 80x24px • Reaction Capsule 236x38px (zoom 1.25x) • Pill badge 22px|
| • Mở file trực tiếp ổ cứng (ShellExecuteW) • Tự tải cache %TEMP% mở tệp nhận • Context menu file  |
+----------------------------------------------------------------------------------------------------+
| PHASE 4 (P1): ĐẤU NỐI 5 LUỒNG ĐỒNG BỘ SERVER & XÃ HỘI BỊ BỎ QUÊN                                  |
| • Unread 2 chiều (/api/conv/getUnreadMark) • Chat ẩn mã PIN server (/api/hiddenconvers)           |
| • Archived chats (/api/archivedchat) • Nhãn sync (/api/convlabel) • Online lastOnline & Mute sync |
+----------------------------------------------------------------------------------------------------+
| PHASE 5 (P1): TIN NHẮN THOẠI NATIVE & DỊCH THUẬT GIỌNG NÓI AI                                      |
| • Thanh ghi âm composer (VU meter cpal) • Gửi voice MP3 (CMD 11148/11169) • Waveform bubble player|
| • Nút AI dịch giọng nói thành văn bản (CMD 13394) inline trong bubble                              |
+----------------------------------------------------------------------------------------------------+
| PHASE 6 (P1/P2): ZCLOUD REHYDRATION, E2EE PUSH (551), RETRY ĐỘNG & BỐ CỤC RESPONSIVE               |
| • Giải mã WebSocket Push CMD 551 (Signal Protobuf) • zCloud On-Demand Rehydration (noise_id)       |
| • Dynamic Retry chunk [10, 35, 40, 45, 50, 60] • Gateway wss://chat-wpa.zalo.me/ • Narrow <720px  |
+----------------------------------------------------------------------------------------------------+
```

---

## II. ĐỀ XUẤT HƯỚNG TRIỂN KHAI CHI TIẾT TỪNG PHASE

### 🔴 PHASE 1 (P0): TRIỆT TIÊU LỖI CHUÔNG VOIP DAI DẲNG & PREFLIGHT

#### 1. Bối cảnh & Nguyên nhân gốc
- Khi có cuộc gọi đến, fork kích hoạt stream âm thanh CPAL đổ chuông. Nếu người dùng nhận cuộc gọi trên điện thoại (Zalo Mobile), server Zalo gửi sự kiện `ReceiveCall` hoặc `answer` qua WebSocket.
- Do hàm `actor_is_self` trong `src/call.rs` và `src/call_lifecycle.rs` so khớp lỏng lẻo (`uidN` / `uidFrom`), client fork nhầm sự kiện nghe nơi khác là của peer, dẫn đến việc tiếp tục đổ chuông ngầm vô tận và không đóng cửa sổ gọi.
- Thiếu watchdog timer: nếu mất mạng khi đang đổ chuông, fork reo mãi mãi đến khi người dùng kill task.
- Chưa có khâu preflight kiểm tra thiết bị mic/loa trước khi gọi đi.

#### 2. Kế hoạch triển khai kỹ thuật
- **Task 1.1 (`src/call.rs`, `src/call_lifecycle.rs`)**:
  - Sửa hàm `actor_is_self`: Kiểm tra đủ 4 trường `fromId`, `userId`, `calleeId`, `uidTo` với UIN phiên hiện tại.
  - Sửa `answered_elsewhere`: Đảm bảo khi nhận `status == 0` kèm `actor_is_self == true` trên call ID đang ringing $\rightarrow$ trả về `true` (dismissed).
- **Task 1.2 (`src/gui/worker.rs`, `src/gui.rs`)**:
  - Khi `answered_elsewhere` trả về `true` hoặc nhận push `ReceiveCall`, `MissCall`, `RejectCall`:
    - Worker gửi `GuiEvent::CallAnsweredElsewhere { owner, call_id }` và `GuiEvent::CallStateChanged { state: "free" }`.
    - UI xử lý trong `gui.rs`: Lập tức gọi `media::stop_ringtone()`, dừng CPAL capture stream, gỡ các texture video, reset attention viewport, đóng dedicated call window, và hiển thị toast: *"Cuộc gọi đã được nhận trên thiết bị khác"*.
- **Task 1.3 (`src/gui/modals.rs`)**:
  - Thêm `ringing_started_at: Option<Instant>` trong `ActiveCall`.
  - Nếu `is_incoming_ringing == true` và `ringing_started_at.elapsed() >= Duration::from_secs(65)`: Tự động kích hoạt cơ chế hủy an toàn, ngắt chuông CPAL và đưa trạng thái về idle.
- **Task 1.4 (`src/gui/modals.rs`, `src/media.rs`)**:
  - Thêm modal `PreflightDeviceDialog`:
    - Mic: Hiển thị thanh VU meter đo cường độ RMS thực tế thời gian thực (đọc từ callback `cpal::Stream`).
    - Loa: Nút "Kiểm tra loa" phát mẫu âm thanh 440Hz / chuông test trong 2 giây.
    - Nút "Bắt đầu gọi" (chỉ gửi `WorkerCmd::StartCall` sau khi người dùng xác nhận) và nút "Hủy".

#### 3. Tiêu chí kiểm chứng
- Unit test: `test_actor_is_self_matches_all_identity_variants`, `test_answered_elsewhere_dismisses_ringing_call`, `test_watchdog_timeout_aborts_ringing`.
- Native check: Gọi từ tài khoản khác, nhận trên điện thoại $\rightarrow$ fork trên PC lập tức im tiếng chuông và đóng modal trong < 200ms.

---

### 🔴 PHASE 2 (P0): TÁI THIẾT KẾ TOÀN DIỆN COMPOSER & ĐÍNH KÈM TỆP (E1 PARITY)

#### 1. Bối cảnh & Nguyên nhân gốc
- Fork hiện tại có thanh công cụ composer chưa chuẩn Zalo Web: thiếu các nút nghiệp vụ, thiếu menu con khi bấm `...` (Giao việc, Bình chọn, Nhắc hẹn), thiếu modal gửi danh thiếp.
- Trong `src/gui/file_drop.rs` (dòng 548-570), code đang chặn người dùng gửi file nếu có văn bản hoặc format (`"Chưa hỗ trợ định dạng hoặc mức Quan trọng/Khẩn cấp cho tệp..."`), gây ức chế lớn cho người dùng.
- Thẻ tệp chờ gửi (`render_attachment_draft`) đang vẽ dạng hình chữ nhật dài `112x82px`, không đúng chuẩn thẻ vuông `72x72px` bo góc 8px của Zalo Web.

#### 2. Kế hoạch triển khai kỹ thuật
- **Task 2.1 (`src/gui/workspace.rs` - Toolbar 9 nút chuẩn & Menu con)**:
  - Cấu trúc thanh toolbar `.chat-input__tools` theo đúng 9 vị trí:
    1. `Sticker` (mở khay sticker).
    2. `Ảnh` (mở dialog chọn image/video).
    3. `Đính kèm File` (dropdown: Chọn File / Chọn Thư mục).
    4. `Gửi danh thiếp` (mở modal danh thiếp chuẩn Zalo với search + filter chips: Tất cả, Khách hàng, Gia đình, Công việc, Bạn bè).
    5. `Chụp màn hình` (Alt+Ctrl+S).
    6. `Định dạng tin nhắn` (Ctrl+Shift+X - mở thanh ruy-băng tầng 2).
    7. `Tin nhắn nhanh` (quản lý /chao, /stk...).
    8. `Gửi số tài khoản` (card chọn tài khoản ngân hàng).
    9. `Tùy chọn thêm (...)` (popup menu con 3 mục: *Giao việc*, *Tạo bình chọn*, *Tạo nhắc hẹn*).
- **Task 2.2 (`src/gui/file_drop.rs` - Gỡ bỏ rào cản chặn gửi file)**:
  - Xóa bỏ khối logic chặn tại dòng 548-570: Cho phép gửi tệp đồng thời với văn bản trong ô soạn thảo (`caption`).
  - Gắn `self.input_message.trim()` vào trường `caption` của `Batch`, gửi cả lô tệp và chú thích văn bản trong một lệnh duy nhất.
- **Task 2.3 (`src/gui/file_drop.rs` - Tái cấu trúc Khu vực Thẻ Tệp Chờ 72×72px)**:
  - Header: Trái hiển thị `"{count} file"`, Phải hiển thị nút text `"Xóa tất cả"`.
  - Thẻ tệp: Chuyển thành thẻ vuông `72px × 72px`, bo góc 8px (`Rounding::same(8.0)`), nền xám nhạt `pal.bg_elevated`, viền mỏng `pal.border`.
  - Icon file trung tâm to rõ, tên file cắt ngắn ở đáy.
  - Nút tròn `(X)` màu xám đậm ở góc trên bên phải xuất hiện khi hover để xóa từng file.
  - Thẻ cuối cùng: Thẻ vuông `72px × 72px` viền nét đứt (`dashed`) chứa icon dấu cộng `+` to ở giữa để chọn thêm tệp vào lô.
- **Task 2.4 (`src/gui/workspace.rs` - Biến đổi Nút Thích $\rightarrow$ Gửi & Smart Features)**:
  - Khi ô nhập rỗng VÀ không có file chờ: Nút góc phải là Thumbs Up (Thích).
  - Khi có ký tự HOẶC có ít nhất 1 file chờ: Chuyển thành nút Gửi xanh Zalo `#0068ff`.
  - Gợi ý sticker nổi khi gõ emoji shortcuts (`:)`, `:((`, `:heart`, `:D`).
  - Menu nhắc tên `@` trong nhóm chat (kèm mục `@All`).

#### 3. Tiêu chí kiểm chứng
- Kéo thả 3 file (.zip, .pdf, .apk) vào composer $\rightarrow$ hiện 3 thẻ vuông 72x72px, nút X xóa được từng file, nút Xóa tất cả dọn sạch.
- Gõ text "Gửi tài liệu này nhé" $\rightarrow$ nút Thích chuyển thành nút Gửi xanh; bấm Gửi gửi thành công cả 3 tệp kèm caption văn bản, không bị báo lỗi chặn.

---

### 🔴 PHASE 3 (P0): TƯƠNG TÁC TIN NHẮN CHUẨN UX ZALO & VÒNG ĐỜI TỆP LOCAL

#### 1. Bối cảnh & Nguyên nhân gốc
- Dwell timer 150ms trong `src/gui/messages.rs` làm người dùng cảm thấy giao diện bị trễ khi rê chuột qua tin nhắn.
- Thanh công cụ hành động tin nhắn hiện tại dùng text unicode thô sơ, thiếu reaction capsule 6 emoji tiêu chuẩn và pill badge đè mép đáy.
- Tệp người dùng tự gửi hiển thị nút "Lưu về máy" vô lý (trong khi tệp đã nằm sẵn trên máy), còn tệp nhận về không tự mở được.

#### 2. Kế hoạch triển khai kỹ thuật
- **Task 3.1 (`src/gui/messages.rs` - Xóa trễ Hover)**:
  - Loại bỏ hoàn toàn bộ đếm 150ms dwell timer; hiển thị thanh thao tác ngay khi bounding box của message bubble được hover.
- **Task 3.2 (`src/gui/messages.rs`, `src/gui/theme.rs` - Thanh công cụ MQA nổi 80×24px)**:
  - Vẽ thanh MQA nổi `80×24px`, bo tròn 12px, nền trắng/dark elevated viền mỏng, nằm cách mép bubble 20px.
  - 3 icon vector: Reply (Mũi tên cong trái), Forward (Mũi tên cong phải), More (Dấu 3 chấm).
- **Task 3.3 (`src/gui/messages.rs` - Reaction Capsule 236×38px)**:
  - Popup capsule `236×38px`, bo góc 19px, hiệu ứng đổ bóng mờ Zalo.
  - 6 emoji chuẩn: 👍, ❤️, 😂, 😮, 😢, 😡. Hover vào emoji nào thì emoji đó phóng to 1.25x mượt mà.
- **Task 3.4 (`src/gui/messages.rs` - Reaction Pill Badge mép đáy)**:
  - Pill badge cao 22px, bo góc 11px, đè 12px lên đáy bubble tin nhắn.
  - Tooltip hiển thị danh sách người thả cảm xúc với tên màu xanh `#0068ff`.
- **Task 3.5 (`src/gui/messages.rs`, `src/gui/workspace.rs` - Chuẩn hóa Vòng đời Tệp)**:
  - Tệp mình gửi (`is_outgoing == true`): Ẩn nút "Lưu về máy". Nhấp đúp mở thẳng file trên ổ cứng bằng Windows API `ShellExecuteW`.
  - Tệp nhận về: Nhấp vào tệp tự động tải về thư mục cache `%TEMP%/ctdozlite-cache/` và kích hoạt ứng dụng mặc định để mở ngay.
  - Menu chuột phải: "Mở tệp", "Mở thư mục chứa", "Sao chép tệp", "Chuyển tiếp".

#### 3. Tiêu chí kiểm chứng
- Rê chuột vào tin nhắn: MQA xuất hiện tức thì không trễ.
- Bấm vào file mình vừa gửi: Windows mở trực tiếp file đó bằng phần mềm mặc định (Notepad, Word, Acrobat...), không hiện hộp thoại tải lại.

---

### 🟡 PHASE 4 (P1): ĐẤU NỐI 5 LUỒNG ĐỒNG BỘ SERVER & XÃ HỘI BỊ BỎ QUÊN

#### 1. Bối cảnh & Nguyên nhân gốc
- Nhiều module API đã được viết (`src/unread.rs`, `src/hidden_chat.rs`, `src/archive.rs`, `src/labels.rs`, `src/last_activity.rs`) nhưng chưa từng được nối vào pipeline khởi động hoặc thiếu UI handler, dẫn đến tình trạng "code mồ côi" không có người gọi.

#### 2. Kế hoạch triển khai kỹ thuật
- **Task 4.1 (`src/unread.rs`, `src/gui/worker.rs`, `src/gui/workspace.rs` - Unread 2 Chiều)**:
  - Startup: Worker gọi `fetch_manual_unread_marks` (`/api/conv/getUnreadMark` trên `tt-convers-wpa.chat.zalo.me`), cập nhật cờ `is_manual_unread: bool` vào `Thread`.
  - Sidebar vẽ chấm tròn đỏ/xanh đậm Zalo.
  - Chuột phải: *"Đánh dấu chưa đọc"* $\rightarrow$ gọi `/api/conv/addUnreadMark`.
  - Khi mở chat: Tự động gửi `/api/conv/removeUnreadMark` gỡ cờ đồng bộ.
- **Task 4.2 (`src/hidden_chat.rs`, `src/gui/worker.rs` - Chat Ẩn Đồng Bộ PIN)**:
  - Startup: Gọi `/api/hiddenconvers/get-all` lấy danh sách ID hội thoại ẩn từ server.
  - Đổi PIN: Gọi `/api/hiddenconvers/update-pin` đồng bộ server.
- **Task 4.3 (`src/archive.rs`, `src/gui/workspace.rs` - Archived Chats Đồng Bộ)**:
  - Gọi `/api/archivedchat/list` (`label-wpa.chat.zalo.me`), lọc bỏ chat lưu trữ khỏi danh sách chính.
  - Thêm tab/filter *"Hội thoại đã lưu trữ"*; chuột phải có *"Lưu trữ"* / *"Bỏ lưu trữ"* (`/api/archivedchat/update`).
- **Task 4.4 (`src/labels.rs`, `src/gui/labels_panel.rs` - Nhãn Hội Thoại Đồng Bộ)**:
  - Gọi `/api/convlabel/get` nạp nhãn từ server. Gán/tạo nhãn gọi `/api/convlabel/update` đồng bộ 2 chiều với SQLite.
- **Task 4.5 (`src/last_activity.rs`, `src/chat.rs` - Online Last Seen & Mute Đồng Bộ)**:
  - Mở chat 1-1: Gọi `read()` (`/api/social/profile/lastOnline`), hiển thị *"Đang hoạt động"* hoặc *"Truy cập X phút trước"*.
  - Nối `/api/conv/getmute`: Hiện icon chuông gạch chéo 🔕 và chặn âm báo khi có tin mới.

#### 3. Tiêu chí kiểm chứng
- Đánh dấu chưa đọc trên fork $\rightarrow$ Zalo Web/Mobile cũng hiện chấm chưa đọc.
- Mở xem chat trên mobile $\rightarrow$ chấm chưa đọc trên fork tự động biến mất.

---

### 🟡 PHASE 5 (P1): TIN NHẮN THOẠI NATIVE & DỊCH THUẬT GIỌNG NÓI AI

#### 1. Bối cảnh & Nguyên nhân gốc
- Fork hiện chỉ có audio capture cơ bản, chưa có trải nghiệm ghi âm tin nhắn thoại inline hoàn chỉnh, chưa hỗ trợ gửi audio nén MP3 chuẩn Zalo và chưa tận dụng API dịch giọng nói AI.

#### 2. Kế hoạch triển khai kỹ thuật
- **Task 5.1 (`src/gui/workspace.rs`, `src/voice.rs` - Thanh ghi âm Composer bám đáy)**:
  - Bấm icon micro: Mở thanh ghi âm thay thế ô nhập text.
  - Hiển thị thời gian thực: Bộ đếm giây (`0:01`, `0:02`...), sóng âm VU meter sống động dựa trên RMS mic buffer.
  - 3 nút điều khiển: "Hủy" (thùng rác), "Nghe thử" (play), "Gửi" (máy bay giấy xanh).
- **Task 5.2 (`src/media.rs`, `src/voice.rs`, `src/gui/worker.rs` - Gửi Voice MP3)**:
  - Nén PCM sang MP3/AAC qua encoder.
  - 1-1 Chat: Gửi qua `/api/message/voicemp3` (Command 11148).
  - Group Chat: Gửi qua `/api/group/sendmp3` (Command 11169).
  - Mã hóa AES tham số với Session Key, đính kèm `fileContent` dạng multipart FormData.
- **Task 5.3 (`src/gui/messages.rs`, `src/ai_features.rs` - Bubble Player & AI Dịch giọng nói)**:
  - Bubble tin nhắn thoại: Nút Play/Pause, dải waveform, thời lượng.
  - Nút icon "AI": Khi bấm, worker gửi yêu cầu Speech-to-Text (Command 13394), kết quả trả về hiển thị transcript ngay bên dưới waveform trong bubble.

#### 3. Tiêu chí kiểm chứng
- Ghi âm 5 giây trên fork và gửi $\rightarrow$ điện thoại nhận được tin nhắn thoại nghe rõ ràng, hiển thị đúng waveform.
- Bấm nút AI trên fork $\rightarrow$ hiển thị chính xác phụ đề văn bản tiếng Việt của đoạn ghi âm.

---

### 🟢 PHASE 6 (P1/P2): ZCLOUD REHYDRATION, E2EE PUSH (551), RETRY ĐỘNG & BỐ CỤC RESPONSIVE

#### 1. Bối cảnh & Nguyên nhân gốc
- Tin nhắn E2EE real-time qua WebSocket frame CMD 551 (`CMD_SIGNAL_RECEIVE_1_1`) chưa được kích hoạt giải mã ngay trong worker.
- Media cũ trên đám mây zCloud chưa được kéo về khi người dùng bấm xem.
- Timeout upload tệp đang dùng hằng số cố định, chưa tận dụng mảng retry động từ server.
- Cửa sổ nhỏ (< 720px) bị vỡ giao diện do sidebar chiếm diện tích cố định.

#### 2. Kế hoạch triển khai kỹ thuật
- **Task 6.1 (`src/socket.rs`, `src/gui/worker.rs`, `src/e2ee.rs` - E2EE Realtime Push CMD 551)**:
  - Trong `src/socket.rs`: Bắt frame WebSocket Push ID 551 (`CMD_SIGNAL_RECEIVE_1_1`).
  - Trong `worker.rs`: Giải mã payload Signal Protobuf với ratchet session keys, ghi vào SQLite và đẩy lên UI ngay lập tức.
- **Task 6.2 (`src/zcloud.rs`, `src/media.rs` - zCloud On-Demand Rehydration)**:
  - Trích xuất `viewer_key` + `enk` từ AUTH GET.
  - Khi xem ảnh/tệp zCloud cũ: Gọi `verifyCloudMedia` và `getDownloadUrlByNoiseIds` bằng `noise_id` để tải về cache và render.
- **Task 6.3 (`src/server_config.rs`, `src/media.rs` - Dynamic Retry 3 Tầng)**:
  - Parse mảng timeout chunk `retry_upload_file_configs` `[10, 35, 40, 45, 50, 60]` và `max_retry` từ `/api/login/setting` vào `ServerConfig`.
  - Áp dụng vào thời gian chờ upload chunk trong `src/media.rs`.
- **Task 6.4 (`src/gui/workspace.rs` - Responsive & Narrow Mode)**:
  - Khi chiều rộng cửa sổ < 720px: Sidebar tự co về 72px (Avatar-only), ẩn tên và preview tin nhắn.
  - Header chat hiển thị nút `<- Quay lại` để chuyển đổi mượt mà giữa danh sách hội thoại và khung chat.

#### 3. Tiêu chí kiểm chứng
- Gửi tin E2EE từ điện thoại sang $\rightarrow$ fork nhận và giải mã hiển thị tức thì qua WebSocket trong < 100ms mà không cần bấm F5 hay chờ sync.
- Co nhỏ cửa sổ về 600px $\rightarrow$ giao diện tự động chuyển sang layout compact không bị tràn thanh cuộn ngang.

---

## III. KẾ HOẠCH HÀNH ĐỘNG SAU TRIỂN KHAI (POST-IMPLEMENTATION HARDENING)

Sau khi hoàn thành mã nguồn của cả 6 Phase, quy trình nghiệm thu và kiểm soát chất lượng sẽ được thực hiện theo đúng tiêu chuẩn kỹ thuật:

```text
[BƯỚC 1: KIỂM TRA BIÊN DỊCH & TOÀN VẸN MÃ NGUỒN CỤC BỘ]
   │
   ├── 1.1: cargo fmt --all (Chuẩn hóa format toàn bộ repository)
   ├── 1.2: cargo check --locked (Bảo đảm 0 lỗi biên dịch, 0 type mismatch)
   ├── 1.3: cargo clippy --all-targets -- -D warnings (Bảo đảm 0 warning clippy)
   └── 1.4: powershell scripts/audit-prepush.ps1 (Cổng kiểm tra hard-gate của repository)
   │
[BƯỚC 2: BẢO VỆ REGRESSION & BỘ KIỂM THỬ TẬP TRUNG]
   │
   ├── 2.1: Chạy toàn bộ 1.110+ unit tests hiện có: cargo test --locked
   ├── 2.2: Bổ sung bộ test hồi quy chuyên biệt cho từng Phase:
   │        • test_call_actor_identity_and_dismissal (Phase 1)
   │        • test_composer_file_batch_caption_unblocked (Phase 2)
   │        • test_reaction_capsule_and_local_file_open (Phase 3)
   │        • test_server_sync_unread_and_archived_reconcile (Phase 4)
   │        • test_voice_recording_and_ai_transcript_payload (Phase 5)
   │        • test_e2ee_cmd_551_push_and_dynamic_retry_config (Phase 6)
   │
[BƯỚC 3: KIỂM CHỨNG THỰC TẾ TRÊN THIẾT BỊ NATIVE WINDOWS 11 ARM64]
   │
   ├── 3.1: Biên dịch bản chạy thử nghiệm: cargo build --locked (target native ARM64)
   ├── 3.2: Khởi chạy ctdozlite.exe và đối chiếu 1-1 với Zalo PC và Zalo Web:
   │        • Test gọi thoại: nghe trên mobile -> fork tắt chuông lập tức.
   │        • Test kéo thả file: hiển thị thẻ 72x72px, gõ caption, bấm gửi thành công.
   │        • Test hover/reaction: capsule 6 emoji nổi bật, zoom 1.25x mượt mà.
   │        • Test đồng bộ: đánh dấu chưa đọc, lưu trữ chat, xem trạng thái online.
   │
[BƯỚC 4: LẬP HỒ SƠ BÀN GIAO & AUDIT CHÉO ĐỘC LẬP]
   │
   ├── 4.1: Reviewer A (Auditor Alpha - Concurrency & DB) thẩm định locking, SQLite WAL, channel.
   ├── 4.2: Reviewer B (Auditor Beta - Protocol Parity & UX) thẩm định schema Zalo và non-blocking.
   ├── 4.3: Cập nhật tài liệu kiểm chứng: docs/research/2026-11-24-zalo-parity-acceptance-dossier.md.
   └── 4.4: Báo cáo nghiệm thu cuối cùng trình người dùng.
```
