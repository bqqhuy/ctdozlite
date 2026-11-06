# Kế hoạch và kiến trúc

Dùng Session/ParamCipher/service map hiện có. UI không làm HTTP, crypto hoặc SQLite.
`account_controls` chỉ nhận state control từ queue control, không diễn giải text chat.
WS và HTTP control fallback dùng cùng bộ invalidation; epoch mới khóa lại PIN và
vô hiệu hóa quyền nhóm. Snapshot quyền chỉ tồn tại trong RAM, không hydrate lại từ đĩa.
Runtime generation là counter duy nhất, không lấy địa chỉ bộ nhớ có thể được tái sử dụng.

`account_metadata` đọc capability độc lập, giữ snapshot hoàn chỉnh trước khi thay UI;
cache theo tài khoản cho phân loại/biệt danh, cập nhật từ control và người gửi chưa rõ.
Đợt rà định kỳ khi yên lặng là 15 phút, không tải lại lịch sử. Chỉ persist thread khi
metadata thay đổi. Các thao tác tài khoản được serialize để disk snapshot cũ không ghi đè ACK mới.

`hidden_chat` giữ verifier/PIN trong zeroizing memory, so sánh constant time; năm lần
sai thì chờ một phút trong runtime. Cache chỉ có membership; không đưa verifier/PIN vào
log/dump/cache. Xem ADR-0034. Thiếu verifier không ngăn đọc chat thường, nhưng không
cho mở khóa/đổi PIN từ dữ liệu chưa xác minh.

Poll detail không có group ID. Xác minh liên hệ qua board/list có group_id, giới hạn
thời gian/trang/cursor, cache binding ngắn hạn theo account/runtime/revision; đọc detail
mới cho trạng thái poll. Ghi chú dùng creator từ danh sách server, không tin creator của UI.

Không đổi schema SQLite, không thêm dependency. Không commit/build/tag/release như
side effect. Nguồn và ngoại lệ: re-notes/34 và re-notes/35.
