# Group send policy — v0.007

Nguồn đối chiếu: metadata getmg-v2 đã dùng trong fork; tài liệu rà PC/Web ở nghiên cứu bổ sung 2026-11-05; upstream PR97 (9a86e69) đọc setting.lockSendMsg, creatorId và adminIds.

Không thêm endpoint hoặc tham số mới. Đường outgoing dùng fetch_group_profiles hiện có để xác minh lại trước khi gửi; không suy vai trò từ tên hoặc vị trí roster. Parser chấp nhận boolean/0/1 và chuỗi 0/1; giá trị khác là chưa biết. Khi khóa, chỉ UID creator/admin được gửi; metadata thiếu ở worker trả lỗi, không giả thành công.

Phạm vi này chưa mô hình hóa mọi quyền cộng đồng, trạng thái bị loại và control đổi quyền. Chưa có nghiệm thu live owner/admin/member trên Windows ARM64. Không thay server ACK hoặc sender ID.
