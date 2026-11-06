# Bằng chứng khởi tạo — 05/11/2026

Đã đọc metadata/diff PR #105 head 3f7c2a9e0. Đây là PR chưa merge; claim test của tác giả
không phải kết quả chạy trên fork. Fork có native folder picker qua rfd, quote/navigation,
forward và kiểm tra tương phản; chưa có bộ chỉnh font/scale/màu trực tiếp tương đương.

Chưa port mã, chưa chạy test ứng dụng hay nghiệm thu UI trong lượt này.
Khi triển khai, ghi commit/working tree, yêu cầu/task, lệnh/kịch bản, kết quả, giới hạn
và thiết bị thật. Không suy số test của PR thành nghiệm thu của fork.

## Triển khai working tree v0.008

Đã thêm font/scale/màu live, picker validation/cancel và cải tiến forward/reply.
Full test Linux qua trước đợt privacy mới; xem [progress](../../docs/releases/v0.008-arm-progress.md).
Các test GUI xác nhận cancel giữ thư mục cũ; theme test kiểm tra contrast bubble và
font/zoom hỏng; các regression composer/quote có sẵn vẫn qua. Chưa xác nhận native DPI.

## Kiểm tra các cửa sổ quản trị mở rộng

Lượt tiếp tục trên working tree `feature/v008-arm`: regression
`gui::v008_dialog_layout_tests::new_management_dialogs_stay_inside_small_light_and_dark_viewports`
đã chạy qua với group settings/QR, profile đã tải và avatar/history, ở 320×240 và
480×360, sáng/tối. Đây là egui headless trên Linux, không thay kiểm chứng native ARM64/DPI.
Regression account/generation của avatar history cũng qua; không gửi yêu cầu tài khoản thật.
