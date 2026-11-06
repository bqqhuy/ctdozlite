# Kế hoạch và Kết quả Nghiệm thu Khối R11

## 1. Tiêu chí kiểm thử tự động (Unit / Integration Tests)
- [x] Test tạo payload và ký `signkey` cho endpoint Contact Card 1-1 và Group (`src/chat.rs::send_contact_card`).
- [x] Test giải mã tin nhắn dạng `chat.contact` và `contact` thành model hiển thị (`contact_and_batch_tests::parse_chat_contact_message`).
- [x] Test phân giải chuỗi JSON string content của contact (`contact_and_batch_tests::parse_contact_json_string_content`).
- [x] Test serialization roundtrip của `ChatMessage` có contact metadata (`contact_and_batch_tests::contact_serialization_roundtrip`).
- [x] Test tạo request batch cho `multi_undo_messages` với descriptor `{ cliMsgId, cliMsgIdUndo, globalMsgId }`.
- [x] Test tính toán zoom, pan offset, duyệt gallery trong Media Viewer (`ImageViewerModal`).

## 2. Tiêu chí kiểm thử trên thiết bị thật (Windows 11 ARM64 Native)
- [ ] Gửi danh thiếp tới tài khoản Zalo PC / Mobile thật: kiểm tra hiển thị đúng tên, avatar, số điện thoại.
- [ ] Thử chọn nhiều tin và chuyển tiếp tới nhóm khác: kiểm tra người nhận xem được đầy đủ các tin.
- [ ] Thử thu hồi nhiều tin cùng lúc: kiểm tra cả hai bên đều thấy tin nhắn biến mất và chuyển thành thông báo đã thu hồi.
- [ ] Mở Media Viewer: phóng to, thu nhỏ, kéo pan, ấn mũi tên trái phải để chuyển ảnh trong hội thoại.
- [ ] Ghi âm một đoạn thoại 5 giây: kiểm tra thanh sóng âm, nghe lại và gửi; tài khoản Zalo chính thức nghe được trọn vẹn.
