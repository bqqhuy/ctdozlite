# Bằng chứng baseline — 05/11/2026

Người dùng báo fork vẫn hiện incoming sau khi đã nhận trên máy khác.
Đã xác nhận trong hội thoại: cuộc gọi cá nhân, nhận trên điện thoại; chưa xác nhận Android/iPhone cho chính ca này.
Đã đọc src/call_lifecycle.rs và các nhánh answer/group_answer, synth request,
CallStateChanged/IncomingCall trong src/call.rs và src/gui.rs.

Helper và unit test đã tồn tại không chứng minh envelope thực đã tới đúng transition.
Chưa có log/fixture của ca lỗi, chưa xác định root cause, chưa sửa mã.
Không suy actor=self khi thiếu thông tin; không sửa bằng cách bỏ qua mọi event answer.

Khi nghiệm thu ghi build, hai thiết bị, cá nhân/nhóm, event metadata đã làm sạch,
thời điểm UI dừng, remote call có tiếp tục và các ca âm tính. Không đưa full VoIP
payload, token, số điện thoại hoặc thông tin media/session vào Git.

## Working tree v0.008

Đã harden transition, bỏ missing-status=>success, owner/call ID ở biên GUI, bounded
terminal history và replay guard. Test GUI/engine helper qua; xem
[re-notes/34](../../re-notes/34-v008-implementation-contracts.md). Chưa có capture của
chính ca lỗi; chưa đánh dấu C-T002/C-T090 hoặc root cause đã xác nhận.

06/11: bổ sung guard CallRejected owner/partner/call ID, bỏ status mặc định,
terminal-before-invite và trạng thái lỗi không tác động cuộc gọi khác. Capture và
nghiệm thu C-T002/C-T090 vẫn mở; không kết luận root cause của thiết bị từ model test.
