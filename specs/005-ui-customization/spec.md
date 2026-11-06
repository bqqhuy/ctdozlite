# Tùy chỉnh giao diện và rà UX theo PR #105

Ngày: 05/11/2026. Bản dự kiến: v0.008-arm. Trạng thái: **đang triển khai**, đã có mã UI và test Linux; chưa nghiệm thu native.
Nguồn: [kế hoạch release](../../docs/releases/v0.008-arm-plan.md), yêu cầu người dùng
đưa các điểm PR #105 vào danh sách. Người dùng đã yêu cầu triển khai v0.008-arm.

## Yêu cầu dự kiến

| ID | Hành vi / tiêu chí nghiệm thu |
| :--- | :--- |
| UI-01 | Chọn/hủy thư mục tải native; đường dẫn đầy đủ xem được; lỗi quyền ghi/thư mục thiếu không làm mất cấu hình tốt và không block UI |
| UI-02 | Font/scale áp trực tiếp, có reset/preset; tiếng Việt/emoji không mất glyph; DPI/cửa sổ nhỏ không mất điều khiển; mở lại giữ đúng lựa chọn |
| UI-03 | Chọn màu primary/accent/bubble và xem preview; chữ, metadata, selection, hover/disabled đọc được ở sáng/tối; dữ liệu cấu hình hỏng không phá theme |
| UI-04 | Reply đúng ID, focus ô soạn, Escape/hủy và nhãn media rõ; double-click chỉ thêm sau khi kiểm tra xung đột chọn chữ và reaction |
| UI-05 | Forward tìm/chọn đúng UID khi tên trùng, Escape hủy đúng modal; ACK chưa rõ không gửi lần hai; bảo vệ nguồn vẫn có hiệu lực |
| UI-06 | Test kiểm tra hành vi và biên tài khoản/cancel, native smoke DPI/sáng-tối; không lấy test tự lặp implementation làm bằng chứng tương thích Zalo |

UI-01/UI-04/UI-05 phần lớn đã có nền, cần rà thay vì thay nguyên module.
Lựa chọn font, khoảng scale và preset cụ thể được đánh giá trong triển khai;
không coi danh sách font trong PR là font được phép đóng gói lại.

## Ngoài phạm vi

Liquid Glass/macOS, thay giao thức forward/quote, lấy cả PR, chống thu hồi hoặc tự đổi
quyền nhận tin người lạ. Phần người lạ được chốt riêng tại [spec 006](../006-stranger-inbox/spec.md), không gộp vào task UI.
