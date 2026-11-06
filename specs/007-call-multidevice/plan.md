# Kế hoạch xử lý cuộc gọi đa thiết bị

## Nền và bằng chứng hiện tại

src/call_lifecycle.rs đã có answered_elsewhere; src/call.rs gọi trong nhánh answer/group_answer.
Helper yêu cầu call ID khác 0, trùng ID, callee chưa connected, status 0; nhóm cần actor self.
CallStateChanged(String) ở biên GUI chưa mang đầy đủ owner/call identity. Đây là quan sát
mã, chưa chứng minh nguyên nhân của báo cáo. UI có xử lý free để đóng controls.

## Thứ tự

1. Tái hiện bằng hai thiết bị; ghi metadata control đã làm sạch, đối chiếu Zalo PC/Web/
   phần native call khi cần. Xác minh envelope/act, status, ID và actor thay vì nới điều kiện đoán.
2. Rà WebSocket và HTTP control extraction: object/string/mảng, alias/kiểu ID và ordering.
   Định danh mobile/web phải có mapping/evidence; không so tên hiển thị hoặc cắt UID tùy ý.
3. Định nghĩa transition terminal theo account/epoch/call ID và trạng thái hiện tại.
   Terminal history chống replay phải có giới hạn/budget, không giữ mãi hoặc chặn gọi mới.
4. Rà biên GUI, IncomingCall đến muộn và nhánh synth request từ answer. Đổi event
   có identity nếu cần; cập nhật tương thích mọi producer/consumer và test.
5. Dọn local UI/ring/media/keepalive đúng cuộc gọi, giữ remote leg; không dùng đường
   EndCall của người dùng để dismiss một cuộc gọi đã nhận nơi khác.
6. Fixture/test end-to-end bridge, native Windows và hai thiết bị. Nếu cần query trạng
   thái server, xác minh API trước; timer chỉ là timeout, không chứng minh accepted elsewhere.

## Giả thuyết cần phân biệt

- Control khác act/status/envelope so với nhánh hiện có hoặc bị extraction bỏ qua.
- Actor/call ID khác kiểu/namespace, nhất là nhóm và mobile UID.
- Terminal đã xử lý nhưng invite/IncomingCall đến muộn tái tạo UI.
- GUI nhận event thiếu identity hoặc state local không khớp coordinator.

Không giả định mọi answer là trả lời ở nơi khác; không bỏ bảo vệ cuộc gọi đi/đã kết nối.
