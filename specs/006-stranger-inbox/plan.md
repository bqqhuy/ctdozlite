# Kế hoạch hộp người lạ

1. Đọc AGENTS/standards, re-notes/32 và các module friendship/unread/presence/outgoing.
2. Xác minh callsite/payload/control của setting 59, respondedByMe/outside và ngoại lệ
   OA/Cloud/group/pin; route trong bảng lệnh không đủ để gọi mutation.
3. Thiết kế state tách friendship và classification; bảo toàn account identity, replay,
   unread/visibility. Nếu cần migration thì viết ADR và migration forward-only.
4. UI hộp riêng/trực tiếp, trạng thái Unknown và các thao tác rõ ý nghĩa; không lặp các
   lệnh gửi khi ACK chưa rõ. Giữ cơ chế outgoing hiện có.
5. Fixture và headless UI trước; nghiệm thu liên thiết bị bằng tài khoản thử được phép.

Được đưa vào phạm vi v0.008, nhưng lượt hiện tại chỉ thống kê/thảo luận. Không viết mã,
đổi server settings hoặc tự mở rộng sang các tính năng khác trong bảng API gap.
