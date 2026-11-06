# Hộp người lạ và trạng thái hội thoại

Ngày: 05/11/2026 (Asia/Saigon). Bản: v0.008-arm.
Trạng thái 06/11: **đã triển khai mã/fixture, chờ nghiệm thu thiết bị**; xem verification.md.
Nguồn: người dùng đồng ý đưa cùng PR #105 vào lượt v0.008.

## Yêu cầu

| ID | Hành vi và nghiệm thu |
| :--- | :--- |
| S-01 | Tách friendship khỏi inbox classification Unknown/Normal/StrangerBox; lỗi metadata hoặc thiếu tên trong danh bạ không tự là người lạ |
| S-02 | Hiển thị hộp người lạ hoặc trực tiếp theo setting/state server đã xác minh; setting hiển thị khác quyền cho phép nhận tin |
| S-03 | Nhận/reconnect/refresh vẫn giữ đủ dữ liệu đã giao; trả lời không đồng nghĩa kết bạn; chỉ chuyển trạng thái theo ACK/evidence, không theo việc mở chat |
| S-04 | Unread, thông báo, typing/seen được rà riêng cho người lạ; tôn trọng privacy server, không tự bật gửi receipt hoặc thay quyền nhận tin |
| S-05 | Phân biệt kết bạn, từ chối lời mời, xóa hội thoại, chặn/bỏ chặn và báo cáo; chỉ triển khai mutation có hợp đồng xác minh, xác nhận thao tác và không tự lặp khi mất ACK |
| S-06 | Trường allow_message_from không được mô tả như privacy server đang hoạt động khi chưa nối API; setting không hỗ trợ phải thể hiện trung thực |

## Tình huống kiểm chứng

Bạn bè / người lạ chưa trả lời / đã trả lời / lời mời gửi-nhận / OA / Cloud / nhóm;
setting hiện trực tiếp bật-tắt; bị chặn/không cho nhận tin người lạ; gửi file bị từ chối;
account switch, offline/reconnect, metadata lỗi và cập nhật từ thiết bị khác.

[Đối chiếu PC/Web](../../re-notes/32-v008-stranger-conversation-review.md).
Không suy quyền từ việc đã nhận một gói hoặc đã bấm trả lời. Không nhắn thử, chặn người
hay đổi privacy tài khoản thật như một phần của test tự động.
