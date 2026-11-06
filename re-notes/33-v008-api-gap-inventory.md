# Phương pháp thống kê API gap — 05/11/2026

Nền fork: commit phát hành 0b1e062. Không sửa mã runtime hoặc gọi API tài khoản.

## Nguồn và cách quét

Hai snapshot đã lưu: PC 26.11.10 compact-app-pc.5c0110a3a8749b3aacde.js và Web
1.c72c0957b455ab70b73c.js. Quét descriptor commandId/path và helper registration
nhận command số + path literal bắt đầu /api/. Loại path ngoài /api/ khỏi tổng.
Mỗi cặp command/path giữ source/offset; hash toàn file nguồn trong JSON để truy vết.

Kết quả: 421 command/path, 421 path duy nhất. 119 path có exact string trong src/**/*.rs.
Đối chiếu chuỗi không xác nhận caller (có cả comment/test), không nhận path dựng động,
HTTP tương đương WebSocket hoặc API nằm chunk/service khác. Không tính tỷ lệ hoàn thiện.
Các route cloudmedia dùng enum ID không thuộc bộ numeric này được ghi riêng theo callsite.

## Rà thủ công

[Feature/API report](../docs/ZALO_FEATURE_API_GAPS.md) đối chiếu source module/consumer,
phân biệt 20 nhóm chưa có luồng tương đương, 12 nhóm có nền/local và các nhóm còn thiếu
bằng chứng. Đây là nhóm phục vụ ưu tiên nghiên cứu, có thể liên quan nhau; không phải
32 tính năng độc lập hoặc cam kết v0.008. Route trong bảng lệnh là bằng chứng sự tồn tại
trong snapshot, không phải schema/method/quyền tài khoản đang dùng được.

Ví dụ tránh kết luận sai:

- autodelete/getConvers không có chuỗi literal nhưng fork dựng path động.
- call.rs có requestsharescreen/group/sharescreen: không gọi signaling là capture/
  encode/transport màn hình đã hoàn thiện.
- local hidden/auto-reply, FTS library, codec history không tương đương hidden-PIN/
  server automation/server media library/E2EE chat.
- zCloud, OA/Business/LAN/transfer cần điều tra riêng, không suy từ service-map/route.

[Index JSON](fixtures/v008-official-route-index.json) chỉ chứa path/cmd/source hash,
không có cookie/key/capture tin người dùng. Mã PC/Web gốc không được chép vào Git.
