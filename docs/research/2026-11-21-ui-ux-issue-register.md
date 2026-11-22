# Sổ đăng ký lỗi UI/UX ZaloLite

Ngày: 2026-11-21

Phạm vi: các lỗi giao diện và trải nghiệm đã được người dùng báo trong phiên làm việc này.

Tài liệu này tách khỏi bản phân tích kiến trúc realtime/sync. Người dùng đã yêu cầu ngắt phần sửa logic để chờ Claude phản biện. Các diff liên quan vì vậy là **sửa dở / tạm dừng chờ phản biện**, không phải đã sửa xong hoặc đã verify.

## 1. Trạng thái composer và gửi tin

### Đang sửa dở — tạm dừng chờ Claude phản biện

- **Outgoing worker/channel:** diff có thêm channel và optimistic `Pending`; ordering, duplicate intents, backpressure, cancellation và retry chưa được phản biện.
- **QueueWait/deadline:** diff bỏ queue timeout như lỗi send và đổi deadline sang worker start; test chưa chứng minh gửi thành công qua transport.
- **Group permission loading:** diff cho phép composer khi snapshot thiếu; policy/cache lifecycle chưa được phản biện.
- **Composer shortcuts:** diff thêm Shift+Enter và Ctrl+Shift+S cho rich text; chưa xác nhận hành vi native/IME.

File liên quan: `src/gui/worker.rs`, `src/outgoing.rs`, `src/service_resolver.rs`, `src/gui/workspace.rs`.

Không mục nào ở trên được xem là hoàn tất. Không tiếp tục mở rộng hoặc đánh dấu hoàn thành trước khi Claude phản biện.

### Chưa nghiệm thu native

- Chưa test lại trên binary đang chạy trong `C:\Apps\ZaloLite`.
- Chưa gửi liên tục trên tài khoản thật sau thay đổi để đo p50/p95.
- Chưa test đồng thời text + file + ảnh + video + cuộc gọi.

## 2. Các lỗi UI/UX đã ghi nhận nhưng chưa sửa

### P0/P1 — ảnh hưởng trực tiếp cảm giác realtime

1. **Tin đến/outgoing bị delay hoặc đứng khi sync đang chạy.**
   - Triệu chứng: bubble đến muộn; tin sau bị tịt; Zalo gốc mượt hơn.
   - Trạng thái: `ĐANG PHÂN TÍCH / CHỜ PHẢN BIỆN`.
   - Nguyên nhân nghi ngờ: WebSocket receive loop còn chờ resolve tên, SQLite persistence và reaction drain. Đây là vấn đề scheduling/kiến trúc, chưa coi là UI-only.

2. **Dòng sync hoặc hoạt động nền tạo dải/khung đen.**
   - Triệu chứng: khung đen xuất hiện khi có dòng sync.
   - Trạng thái: `CHƯA SỬA`.
   - Cần kiểm tra: vùng status/banner, fill màu theo dark/light palette, clipping, min height và repaint khi sync status đổi.

3. **Khung soạn thảo cách đáy quá xa.**
   - Triệu chứng: khoảng trống dưới composer lớn hơn Zalo PC.
   - Trạng thái: `CHƯA SỬA`.
   - Cần kiểm tra: outer panel bottom padding, `ScrollArea`, reserved toolbar rows, safe-area/window margin và composer frame margin.

### P1 — tìm kiếm và group presentation

4. **Ô tìm kiếm kéo giãn sidebar và tạo dải đen.**
   - Triệu chứng: nhập search làm xuất hiện dải đen; ô search tự kéo giãn khung trái.
   - Trạng thái: `CHƯA SỬA`.
   - Cần kiểm tra: layout width khi search active, `TextEdit::singleline`, parent `available_width`, background frame và min/max width của sidebar.

5. **Group chưa có avatar không dùng avatar ghép.**
   - Triệu chứng: group không đặt avatar không hiển thị avatar ghép từ trưởng nhóm/phó nhóm/thành viên hoạt động gần đây như Zalo PC, ví dụ “Tổ dân phố 37”.
   - Trạng thái: `CHƯA SỬA`; parity contract cần đối chiếu thêm.

6. **Tên danh bạ không đồng bộ xuyên suốt group.**
   - Triệu chứng: chat cá nhân có tên đã sync nhưng trong group vẫn hiện tên khác, ví dụ “TNR chị Huệ” và “Hue Nguyen” trong group CSBH TMOP.
   - Trạng thái: `CHƯA SỬA`.
   - Cần kiểm tra: identity map sender UID → display name, nguồn profile cache, thứ tự resolve group members và việc parser dùng alias cá nhân hay profile name.

7. **Group reaction thiếu danh tính người gửi.**
   - Triệu chứng: chỉ hiện thông báo chung kiểu “Thành viên reaction?”, không có tên người reaction.
   - Trạng thái: `CHƯA SỬA`.
   - Cần kiểm tra: reaction event có sender UID/display name không, pipeline có lưu target/sender hay chỉ cập nhật aggregate, UI có ưu tiên reaction notification sai hay không.

8. **Reaction đang lấn át tin nhắn.**
   - Triệu chứng: thông báo reaction được ưu tiên hơn nội dung chat.
   - Trạng thái: `CHƯA SỬA`.
   - Hướng UX cần xác định: tin nhắn mới giữ vị trí timeline chính; reaction là metadata/sub-event hoặc compact notification.

### P2 — typography, emoji và nhận diện

9. **Emoji hiển thị xấu, trái tim đen.**
   - Triệu chứng: heart và emoji không giống Zalo gốc, màu/shape sai.
   - Trạng thái: `CHƯA SỬA`.
   - Cần kiểm tra: font fallback Windows ARM64, emoji presentation selector, màu glyph native, fallback font và cách render `RichText`/message text.

10. **Khoảng cách emoji với ký tự liền cạnh quá xa.**
    - Triệu chứng: emoji tạo khoảng cách lớn trong bubble.
    - Trạng thái: `CHƯA SỬA`.
    - Cần kiểm tra: font metrics, grapheme measurement, fallback run và line layout; không nên sửa bằng cách cắt chuỗi hoặc chèn khoảng trắng thủ công.

11. **Tên hiển thị “Bống” khác Zalo gốc.**
    - Triệu chứng: tên contact/group member không khớp giữa Zalo PC và fork.
    - Trạng thái: `CHƯA SỬA`.
    - Cần phân biệt: tên riêng do người dùng đặt, tên profile server, alias trong danh bạ và tên snapshot cũ.

## 3. Những phần không được trộn vào UI backlog

Các mục sau thuộc bản phân tích kiến trúc và đang chờ Claude phản biện:

- Tách WebSocket realtime ingress khỏi SQLite/durable writer.
- Tách outgoing scheduler, media scheduler và history/sync scheduler.
- Queue/cursor ordering, replay khi persistence fail.
- Priority giữa incoming/outgoing/call và history sync.
- Fairness, backpressure và giới hạn số task.
- Kiểm tra media upload/download có chặn realtime hay không.

Chúng có thể gây ra triệu chứng UI delay, nhưng chưa được đánh dấu là “sửa UI”.

## 4. Ma trận trạng thái nhanh

| Mục | Đã sửa code | Chưa sửa | Chờ Claude | Native test |
|---|---:|---:|---:|---:|
| Enter gửi / Shift+Enter xuống dòng | Patch dở |  | Chờ Claude | Chưa |
| Ctrl+Shift+S rich text editor | Patch dở |  | Chờ Claude | Chưa |
| Bubble outgoing xuất hiện ngay | Patch dở |  | Chờ Claude | Chưa |
| QueueWait giả thành lỗi | Patch dở |  | Chờ Claude | Chưa |
| Quyền group snapshot chưa tải | Patch dở |  | Chờ Claude | Chưa |
| Dải đen khi sync |  | Có |  | Chưa |
| Search kéo giãn sidebar/dải đen |  | Có |  | Chưa |
| Composer cách đáy |  | Có |  | Chưa |
| Avatar ghép group |  | Có | Có parity gap | Chưa |
| Tên member trong group |  | Có | Có identity gap | Chưa |
| Reaction có tên/ưu tiên |  | Có | Có pipeline gap | Chưa |
| Emoji/heart/font metrics |  | Có |  | Chưa |
| Tên “Bống” |  | Có | Có profile/alias gap | Chưa |
| Realtime vs history scheduling |  |  | Có | Chưa |

## 5. Files hiện đang thay đổi và ý nghĩa

- `src/gui/worker.rs`: kênh outgoing riêng, optimistic outgoing event.
- `src/outgoing.rs`: bỏ queue wait hard failure, deadline tính từ send start.
- `src/service_resolver.rs`: đổi timing context sang send start.
- `src/gui/workspace.rs`: group permission fail-open khi snapshot thiếu; composer shortcuts và Shift+Enter.
- `re-notes/44-map15-network-send.md`: ghi rõ fork deviation của text budget.
- `docs/research/2026-11-21-realtime-vs-history-architecture-review.md`: bản kiến trúc chờ Claude phản biện.

## 6. Verification hiện tại

- `cargo fmt --all -- --check`: đã qua.
- `git diff --check`: đã qua.
- `cargo test`: chưa chạy được vì dependency `ring` không tìm thấy `clang.exe` trong môi trường hiện tại.
- Native Windows ARM64 với binary `C:\Apps\ZaloLite`: chưa nghiệm thu sau thay đổi.
