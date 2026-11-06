# Kế hoạch và Kiến trúc Khối R11

## 1. Kiến trúc phân tầng

### 1.1. Chia sẻ danh thiếp (A17 / M-01)
- Tận dụng `src/contact_actions.rs` và `src/chat.rs`.
- Endpoint:
  - 1-1: `CHAT 11774 /api/message/contact`
  - Group: `GROUP 11827 /api/group/contact`
- Gói tin payload: `{ toId, contactId, contactName, phone, avatar, clientMsgId }`.
- UI:
  - Menu đính kèm (`+`) $\rightarrow$ "Gửi danh thiếp".
  - Dialog chọn liên hệ từ danh sách `self.threads` (bạn bè đã có).
  - Renderer `chat.contact` trong `workspace.rs`: Vẽ thẻ card nổi có bo góc, avatar tròn, tên đậm, nút "Nhắn tin" (chuyển thread ngay lập tức).

### 1.2. Thao tác chọn nhiều tin (B06 / M-02)
- State trong `Workspace`:
  - `batch_selection_mode: bool`
  - `selected_msg_ids: HashSet<String>`
- Menu context chuột phải trên bong bóng chat: "Chọn nhiều tin" $\rightarrow$ bật `batch_selection_mode = true`.
- Renderer: Thêm checkbox tròn ở lề trái mỗi message row.
- Floating Action Bar ở đáy màn hình chat:
  - Số lượng tin đã chọn.
  - Nút "Chuyển tiếp (N)": Mở modal chọn người nhận $\rightarrow$ gửi `WorkerCmd::BatchForward`.
  - Nút "Thu hồi (N)": Kiểm tra `all(is_self)` $\rightarrow$ xác nhận $\rightarrow$ gửi `WorkerCmd::BatchRecall`.
  - Nút "Hủy": Thoát selection mode và xóa `selected_msg_ids`.
- Worker backend:
  - `WorkerCmd::BatchForward { target_thread_id, msg_ids }`
  - `WorkerCmd::BatchRecall { thread_id, msg_ids }`
  - Gọi endpoint `mforward` (12440/12442) và `multi_undo` (12115/12116), xử lý kết quả và phát `WorkerEvent`.

### 1.3. Media Viewer nâng cao (B12 / M-03)
- Nâng cấp `render_image_preview` trong `workspace.rs`:
  - Trạng thái `MediaViewerState`: URL ảnh hiện tại, danh sách URL trong thread, index hiện tại, zoom factor (`1.0..=5.0`), pan offset `(dx, dy)`.
  - Điều khiển bằng chuột: cuộn bánh xe để zoom in/zoom out, kéo chuột trái để pan khi zoom > 1.0.
  - Phím điều hướng: Mũi tên Trái/Phải để chuyển ảnh kế trước/tiếp theo.
  - Nút chức năng trên thanh header của viewer: Tải về (`Save`), Sao chép vào clipboard (`Copy Image`), Đóng (`Esc`).

### 1.4. Ghi âm và gửi tin nhắn thoại (A16 / M-04)
- Thu âm qua `cpal` / WASAPI trong nền, buffer lưu trữ các mẫu PCM âm thanh.
- Gửi lệnh `WorkerCmd::SendVoiceMessage { thread_id, duration_ms, pcm_data }`.
- Worker encode sang format Zalo yêu cầu và upload qua endpoint `voicemp3` / `sendmp3`.

### 1.5. Tìm kiếm Sticker / GIF (B08 / M-05)
- Thêm ô tìm kiếm trong `sticker_picker.rs`.
- Tìm kiếm tức thời qua cache hoặc query route `11747 /api/message/sticker/search`.

## 2. Trình tự thực hiện
1. **Bước 1**: Triển khai M-01 (Chia sẻ Danh thiếp) & M-02 (Batch Forward / Recall) - ít rủi ro về phần cứng âm thanh/video nhất, có thể kiểm thử logic/network ngay.
2. **Bước 2**: Triển khai M-03 (Media Viewer nâng cao với Zoom/Pan/Gallery).
3. **Bước 3**: Triển khai M-04 (Voice Recorder & Send) và M-05 (Sticker Search).
