# Đặc tả Khối R11 — Nội dung phong phú, Media & Tin nhắn mở rộng (v0.009-arm)

Ngày: 06/11/2026 (Asia/Saigon).
Phạm vi: R11 trong roadmap v0.009-arm (`A16, A17, B06, B08, B09, B12`).
Đối chiếu: Zalo PC 26.11.10 (`compact-app-pc.5c0110a3a8749b3aacde.js`) và Zalo Web (`1.c72c0957b455ab70b73c.js`).

| ID | Yêu cầu |
| :--- | :--- |
| **M-01 (A17)** | Chia sẻ danh thiếp liên hệ (Contact card): Hỗ trợ gửi contact card cá nhân tới cuộc trò chuyện 1-1 và nhóm qua route CHAT 11774 `/api/message/contact` và GROUP 11827 `/api/group/contact`. Render tin nhắn dạng `chat.contact` đầy đủ avatar, tên, số điện thoại (nếu có quyền), nút nhắn tin/mở thông tin liên hệ. |
| **M-02 (B06)** | Thao tác chọn nhiều tin nhắn (Batch forward / Multi-recall): Chế độ `SelectionMode` trong khung chat cho phép chọn nhiều tin nhắn bằng checkbox. Hỗ trợ chuyển tiếp hàng loạt (`mforward` 12440/12442) và thu hồi hàng loạt tin của chính mình (`multi_undo` 12115/12116). |
| **M-03 (B12)** | Trình xem media tương tác (Media Viewer): Cho phép xem ảnh/video phóng to với pan/zoom mượt mà bằng chuột, xoay ảnh, nút tải về (Save as), sao chép vào clipboard (`Ctrl+C`), duyệt ảnh trước/sau (Next/Previous gallery) trong cùng luồng hội thoại. |
| **M-04 (A16)** | Ghi âm và gửi tin nhắn thoại (Voice Recorder): Ghi âm qua `cpal`/WASAPI trên Windows 11 ARM64, đo mức âm lượng thời gian thực (waveform preview), cho phép nghe thử hoặc hủy; upload và gửi qua FILE 11148 `/api/message/voicemp3` và 11169 `/api/group/sendmp3`. |
| **M-05 (B08)** | Tìm kiếm và gợi ý Sticker / GIF: Hỗ trợ tìm kiếm kho sticker theo từ khóa tiếng Việt qua 11747 `/api/message/sticker/search`, tải bất đồng bộ không chặn luồng vẽ UI. |
| **M-06 (B09)** | Tín hiệu chia sẻ màn hình trong cuộc gọi: Tiếp tục hoàn thiện signaling `requestsharescreen` và bộ thu nhận khung hình chia sẻ màn hình. |

## Ranh giới và Invariants

1. **Không chặn UI**: Mọi tác vụ I/O (thu âm, mã hóa, upload file, tải sticker, batch API) phải chạy qua Tokio Worker via `WorkerCmd`.
2. **Bảo toàn định danh**: ID tin nhắn và contact UID phải được giữ nguyên vẹn, không parse lỏng lẻo hay format sai lệch.
3. **Kiểm tra quyền cục bộ trước khi gửi**:
   - Thu hồi hàng loạt (`multi_undo`) chỉ khả dụng khi 100% tin được chọn do chính tài khoản hiện tại gửi (`is_self == true`) và chưa bị xóa trước đó.
   - Chuyển tiếp tin nhắn tuân thủ chính sách bảo vệ nguồn nếu có.
