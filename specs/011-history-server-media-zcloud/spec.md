# Đặc tả Kỹ thuật Spec 011: Lịch sử, Thư viện Server Media và zCloud (v0.010-arm)

Ngày: 07/11/2026 (Asia/Saigon)
Khối triển khai: **R14**
Mã phạm vi Parity: **A20, B05, B11**

## 1. Mục tiêu và Đối chiếu Zalo gốc

### 1.1. A20 — zCloud media / Khôi phục dữ liệu Cloud
- **Zalo gốc**: Zalo PC và Zalo Web hỗ trợ dịch vụ lưu trữ đám mây zCloud cho các tệp/ảnh/video lớn hoặc có tuổi thọ cao.
  - Khi xem hoặc tải tệp zCloud, client lấy khoá xác thực qua `/api/getCloudViewerKey` hoặc `/cloudmedia/queue/pc/verify`.
  - Gọi `/cloudmedia/downloadurls/pc/v1` để nhận signed URLs.
  - Trạng thái rõ ràng: phân biệt tệp có trên zCloud (`Available`), tệp chưa lưu lên Cloud (`NotInCloud`), hoặc tệp đã hết hạn trên server (`Expired`).
- **ctdozlite**: Cung cấp module `src/zcloud.rs` với data structures `CloudViewerKey`, `CloudMediaItem`, `CloudFileStatus` và các API gọi service zCloud an toàn.

### 1.2. B05 — Kho ảnh, video, tệp tin và liên kết server
- **Zalo gốc**: Panel thông tin hội thoại (bên phải) trên Zalo PC có mục "Kho lưu trữ" phân chia 3 tab riêng biệt:
  1. Hình ảnh / Video (Photo / Video).
  2. Tệp tin (Files) với tên tệp, kích thước, người gửi, ngày gửi.
  3. Liên kết (Links) với tiêu đề, URL trích xuất.
  - Giao thức: Nhóm dùng `/api/mediastore/list` (command 11841..11843); 1-1 dùng `/api/media/oneone/list` (command 12220..12222).
  - Tách biệt rõ ràng nguồn: Chỉ báo dữ liệu lấy từ Server Media Library khác với kết quả tìm kiếm SQLite cục bộ.
- **ctdozlite**: Cung cấp module `src/server_media.rs` với bộ parser chuẩn Zalo API, hỗ trợ phân trang `page/count`, cờ `has_more` và bộ lọc theo loại media.

### 1.3. B11 — V1/BinNet và Khôi phục an toàn Desktop V2
- **Zalo gốc**: Sao lưu và khôi phục định dạng Desktop V2 (SQLite nén + assets) và giao thức đồng bộ điện thoại qua BinNet/V1.
  - An toàn giao dịch: Kiểm tra tính toàn vẹn (integrity check), tạo snapshot dự phòng và tự động rollback nếu gặp lỗi hoặc người dùng bấm Huỷ (`Cancel`).
  - Bảo toàn định dạng: Giữ nguyên `raw/Unknown` khi gặp opcode chưa hỗ trợ, không đoán mò UID.
- **ctdozlite**: Cải tiến `src/phone_sync/import.rs` và `src/phone_sync/codec.rs` hỗ trợ snapshot rollback an toàn và parser BinNet giữ nguyên raw bytes.
