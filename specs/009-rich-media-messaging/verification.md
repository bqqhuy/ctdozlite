# Kế hoạch và Kết quả Nghiệm thu Khối R11

## 1. Tiêu chí kiểm thử tự động (Unit / Integration Tests)
- [x] Test tạo payload và ký `signkey` cho endpoint Contact Card 1-1 và Group (`src/chat.rs::send_contact_card`).
- [x] Test giải mã tin nhắn dạng `chat.contact` và `contact` thành model hiển thị (`contact_and_batch_tests::parse_chat_contact_message`).
- [x] Test phân giải chuỗi JSON string content của contact (`contact_and_batch_tests::parse_contact_json_string_content`).
- [x] Test serialization roundtrip của `ChatMessage` có contact metadata (`contact_and_batch_tests::contact_serialization_roundtrip`).
- [x] Test tạo request batch cho `multi_undo_messages` với descriptor `{ cliMsgId, cliMsgIdUndo, globalMsgId }`.
- [x] Test tính toán zoom, pan offset, duyệt gallery trong Media Viewer (`ImageViewerModal`).
- [x] Test RIFF WAV 44-byte header generation, RMS computation, và tỷ lệ resample 16kHz (`src/voice.rs::tests`).
- [x] Test tìm kiếm từ khóa sticker tiếng Việt và chuyển đổi tab Search (`src/gui/stickers.rs`).
- [x] Test tín hiệu chia sẻ màn hình 1-1 (12064) và nhóm (12044) (`src/call.rs::screen_sharing_signal_payloads_match_zalo_routes`).

## 2. Tiêu chí kiểm thử trên thiết bị thật (Windows 11 ARM64 Native)
- [ ] Gửi danh thiếp tới tài khoản Zalo PC / Mobile thật: kiểm tra hiển thị đúng tên, avatar, số điện thoại.
- [ ] Thử chọn nhiều tin và chuyển tiếp tới nhóm khác: kiểm tra người nhận xem được đầy đủ các tin.
- [ ] Thử thu hồi nhiều tin cùng lúc: kiểm tra cả hai bên đều thấy tin nhắn biến mất và chuyển thành thông báo đã thu hồi.
- [ ] Mở Media Viewer: phóng to, thu nhỏ, kéo pan, ấn mũi tên trái phải để chuyển ảnh trong hội thoại.
- [ ] Ghi âm một đoạn thoại 5 giây: kiểm tra thanh sóng âm, nghe lại và gửi; tài khoản Zalo chính thức nghe được trọn vẹn.

## Kiểm chứng tiếp tục ngày 22/11/2026

Working tree trên `8e235f7`: bộ thư viện trước delta cuối đạt 1048 passed,
0 failed, 11 ignored; sau sửa filter strip và thời điểm assertion smoke,
10 regression composer/sidebar đạt. Clippy all-targets, fmt/diff check và build
debug ARM64 đạt; ba renderer smoke dữ liệu giả đã chạy và xem ảnh.
Chi tiết, lỗi đã tái hiện và giới hạn trong
[bản ghi popup/contact/voice/composer](../../docs/research/2026-11-22-popup-contact-voice-composer.md#tiếp-tục-kiểm-chứng-ngày-22112026).
Không đóng R11-T09 hoặc tiêu chí ghi âm/gửi voice từ kiểm chứng player incoming,
fixture decoder hay ảnh renderer. Công cụ UI tương tác hiện lỗi khởi tạo sandbox;
voice/AI server và phonebook với tài khoản thật còn cần nghiệm thu.

## Delta gửi tệp 204 ngày 22/11/2026

Sửa regular-file fallback theo E2 PC, thêm ZIP vào draft qua worker và chẩn
đoán upload/submit. Lib 1052 passed / 11 ignored; audit_regressions 33 passed;
Clippy, fmt, diff check và build debug ARM64 qua. Renderer smoke scale 1 đã xem;
scale 1.5 còn lỗi layout header/timeline. Chưa gửi/tải lại trên server thật và
không đóng tiêu chí native từ các kiểm thử này. Bằng chứng và ma trận còn thiếu:
[bản ghi xử lý 204](../../docs/research/2026-11-22-attachment-204-remediation.md).

## M-07: bubble tệp trước I/O — 22/11/2026

R11-T10 có mã: công bố toàn bộ lô trước admission/SQLite/preparation, giữ ID và
account/runtime/thread; bỏ cảnh báo thiếu URL đối với intent attachment cục bộ.
Test lô 20 đường dẫn chưa tồn tại qua. Lib: 1053 passed, 0 failed, 11 ignored;
Clippy all-targets -D warnings, build debug ARM64, fmt/diff check qua.
Renderer smoke dữ liệu giả exit 0 và đã xem ảnh; không thay nghiệm thu thời gian
hiện bubble trên GUI thật. R11-T10 giữ chưa đóng vì phần nghiệm thu này còn thiếu.
