# ADR-0011: Ưu tiên ảnh đang nhìn thấy

- Status: accepted
- Date: 2026-11-02

## Context

Render lịch sử/avatar ngoài clip gây nạp hàng loạt ảnh không được nhìn thấy. Avatar, sticker, prefetch và ảnh chat cùng tranh bốn lượt mạng. Hàng đợi đầy bị coi như tải lỗi và đợi 10 giây. Cache được quét/sắp xếp toàn bộ sau mỗi ảnh trước khi gửi texture tới UI. Lệnh ảnh chung hàng đợi với ghi lịch sử khiến đồng bộ ảnh hưởng phản hồi ảnh.

## Decision

Một kênh ảnh bounded riêng trong cùng runtime worker tiếp nhận ảnh độc lập lệnh SQLite. Chỉ yêu cầu ảnh trong clip và khoảng đón trước 160px (avatar 64px). Không bỏ phần layout tin cũ để tránh thay đổi chiều cao/cuộn. Avatar/sticker nền có hai slot trong tổng bốn slot mạng; ảnh đang xem có thể dùng các slot còn lại. Hai hàng admission bounded giữ sự cô lập foreground/background; texture và kích thước giải mã vẫn có giới hạn.

Prefetch đi qua cùng đường tải private cache, cùng single-flight theo account/key. Drop/abort giải phóng flight. Đọc/giải mã cache sticker ngoài Tokio core threads. Tràn hàng đợi được thử lại cục bộ sau 250ms, khác lỗi tải thật 10 giây. Không tăng số request tải ảnh bằng retry mù hoặc tạo HTTP client riêng.

Ghi cache nguyên tử vẫn trước delivery để giữ bền vững. Dọn cache được coalesce tối đa một lần/10 giây, chạy riêng khỏi delivery; disk budget 512 MiB/account là mục tiêu sau cleanup và có thể vượt tạm giữa các lần. Một cleanup chạy tại một thời điểm. Không đổi chất lượng ảnh gốc hay tải toàn bộ media lịch sử.

## Verification

Test egui xác nhận sticker ngoài clip không tạo yêu cầu, trở lại clip mới yêu cầu. Test flight gộp prefetch/render theo account. Test semaphore xác nhận hai background chậm vẫn để slot foreground. Benchmark kiểm soát mô phỏng bốn background chiếm 50ms/slot, năm lần trước/sau; không phải đo CDN/điện thoại hay cam kết tăng tốc trên mạng thật. Kiểm tra screenshot photo, quote và các tiện ích trên headless/native CI.

## Rollback

Có thể khôi phục đường dispatch media cũ mà không đổi schema hoặc xóa cache. Giữ giới hạn giải mã và account isolation.
