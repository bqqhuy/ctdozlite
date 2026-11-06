# Bằng chứng baseline

Ngày: 05/11/2026. Chỉ đọc source/snapshot, chưa test tài khoản thật hoặc sửa mã.
[re-notes/32](../../re-notes/32-v008-stranger-conversation-review.md) ghi offset và điều chưa biết.

- Đã thấy classifier PC/Web phân biệt người lạ/hộp riêng và setting 59 hiện trực tiếp.
- Fork có friendship banner, chưa có state inbox tương ứng.
- Chưa xác minh đầy đủ API mutation, cập nhật liên thiết bị hay trường hợp server không giao tin.
- Không dùng kết quả Cargo/native CI v0.007 để kết luận tính năng mới đã hoàn tất.

Khi thực hiện: ghi commit, task/yêu cầu, fixture hoặc kịch bản, kết quả và giới hạn;
không lưu cookie/key/tin riêng tư vào Git.

## Working tree v0.008 — 06/11/2026

Đã có StrangerBox với classification đầy đủ, metadata/cache/lifecycle, UI filter,
ACK promotion và gate notification/typing. Unknown không bị suy thành người lạ;
Cloud lấy identity từ login, không dựa nickname. Privacy, block/unblock/report có
confirmation/readback; không tự gửi seen receipt hay đổi quyền nhận tin từ setting hiển thị.
Nguồn/fixture: re-notes/34–35, src/inbox.rs, src/account_metadata.rs và test GUI/live_updates.
Kết quả lệnh cuối: docs/releases/v0.008-arm-progress.md.
S-T090 còn mở: chưa nghiệm thu trên tài khoản/thiết bị, server có thể không giao đủ metadata.
