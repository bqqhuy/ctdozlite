# WebSocket TLS boundary — 2026-11-19

## Bằng chứng và đối chiếu

Parity with Zalo PC/Web (source: re-notes/02_chat_data.md §5–6): endpoint
`zpw_ws` được ghi nhận là `wss://`; bundle dùng danh sách endpoint này để
khởi tạo WebSocket. Lượt sửa này không có capture mới và không xác nhận thêm
schema hoặc hành vi server.

Audit source phát hiện `apply_server_info` còn nhận `ws://`, trong khi
`socket::run_once` chỉ kiểm tra host/DNS trước khi gắn thông tin phiên.
Nếu cấu hình, phiên đã lưu hoặc override `ZCALL_WS` chứa endpoint không TLS,
cookie có thể đi qua kết nối plaintext. Đây là điều kiện cấu hình không an toàn,
không phải bằng chứng bên thứ ba đang khai thác phiên thật.

## Sửa phòng vệ

- Discovery chỉ giữ endpoint `wss://`; danh sách chỉ chứa giá trị không hợp lệ
  không thay thế danh sách đang có.
- Ràng buộc TLS được kiểm tra lần nữa ở đường tạo request cuối, trước DNS và
  trước gắn cookie/header. Nó áp dụng cho cả discovery, phiên đã lưu và override.
- Không thay đổi frame, params, cookie, query/path hoặc danh sách host hợp lệ.
- Không có ngoại lệ plaintext cho test hoặc override; test kiểm tra thuần offline.

## Kiểm chứng

Test hồi quy trong `session.rs` kiểm tra discovery bỏ endpoint plaintext và giữ
endpoint WSS; test trong `socket.rs` kiểm tra request plaintext bị từ chối và
WSS giữ nguyên path/query. Kết quả chạy test do agent chính ghi vào báo cáo audit.
Không thử trên tài khoản thật hoặc gửi thông tin phiên qua mạng.
