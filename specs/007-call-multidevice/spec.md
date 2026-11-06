# Cuộc gọi được nhận ở thiết bị khác

Ngày: 05/11/2026 (Asia/Saigon). Ưu tiên: **P0 của v0.008**, chưa sửa mã trong lượt lập kế hoạch.
Báo cáo: người dùng nhận cuộc gọi ở thiết bị khác nhưng fork vẫn hiện màn hình cuộc gọi đến.
Người dùng đã xác nhận: **cuộc gọi cá nhân, nhận trên điện thoại**. Đây là ca nghiệm thu chính; nhóm và các thiết bị khác là hồi quy.

## Yêu cầu

| ID | Hành vi / nghiệm thu |
| :--- | :--- |
| C-01 | Khi có bằng chứng cuộc gọi đang chờ của chính tài khoản đã được nhận ở thiết bị khác, fork dừng reo và đóng mọi UI đến của đúng cuộc gọi, không cần bấm từ chối |
| C-02 | Chỉ dọn tài nguyên local; không gửi end/cancel/reject làm gián đoạn cuộc gọi đang tiếp tục ở máy khác |
| C-03 | Terminal event đến trước invite hoặc invite bị replay không làm hiện lại cuộc gọi đó; gọi mới khác ID vẫn hoạt động |
| C-04 | Event gắn tài khoản/phiên/call ID; sự kiện cũ không đóng nhầm cuộc gọi mới, tự nhận ở fork hoặc cuộc gọi đi đã kết nối |
| C-05 | Với nhóm, thành viên khác nhận không mặc nhiên là tài khoản mình nhận; không đóng group call chỉ từ actor không rõ hoặc từ UID chưa ánh xạ |
| C-06 | UI phân biệt đã nhận ở nơi khác với bỏ lỡ/từ chối/kết thúc; nhạc reo, popup/tray, camera/mic/keepalive local không bị giữ lại |
| C-07 | Có fixture theo envelope thật đã làm sạch, test state-machine/GUI và nghiệm thu hai thiết bị; log chỉ giữ metadata cần thiết, không full payload/session material |

## Tình huống bắt buộc

1. Fork reo; nhận ở điện thoại hoặc Zalo PC/Web khác; fork dừng reo/UI, đầu kia vẫn đàm thoại.
2. Nhận ở fork; event answer lặp không đóng cuộc gọi local đang kết nối.
3. Cuộc gọi đi được đối tác nhận: chuyển connected bình thường.
4. Nhóm: chính mình nhận ở nơi khác / người khác trong nhóm nhận / rời nhóm call.
5. Answer/end trước invite, duplicate/replay, account switch, reconnect; hai call ID gần nhau.
6. Thiếu actor/status/call ID: không đoán thành accepted elsewhere, không dùng timeout để ghi giả kết quả.
