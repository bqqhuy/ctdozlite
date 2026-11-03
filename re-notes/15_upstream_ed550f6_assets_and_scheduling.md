# Bổ sung upstream ed550f6 — 03/11/2026

Nguồn đã đọc: `ctdoteam1110/ctdozlite` tại `ed550f6b5a69cbe154d68c28874104d7368d36bf`. Các file `src/assets/ami_fat_cat/*.{png,gif}` được sao chép nguyên bytes; nhãn/ID đối chiếu `src/gui/stickers.rs` của commit đó.

Upstream dùng category `99901`, ID `999001`…`999015` làm selector cục bộ, ghi GIF và gọi API upload attachment. Đây không phải bằng chứng các ID đó là sticker thật của máy chủ. Fork giữ selector để render PNG nhúng, dùng đường attachment hiện có với client ID ổn định cho cả gửi và chuyển tiếp. Không thêm host, trường mã hóa, thuật toán ký hoặc endpoint mới. Không đưa nhánh lỗi upstream phát `StickerSent` vào fork: lỗi tiếp tục đi qua `DeliveryState::Failed` và retry.

Hộp hẹn giờ chỉ tạo `ToolsCommand::Schedule` của fork, dữ liệu gắn owner/thread như trước. Không port DB tương đối/global scheduler hoặc các tab Danh bạ chưa có dữ liệu. Những phần đã nhận từ commit này trong `15344dd` không được nhập lặp.

Kiểm thử dùng asset nhúng, timestamp tổng hợp và fake account; không dùng tài khoản Zalo thật. Xem ADR-0016 và báo cáo đối chiếu issue/PR ngày 03/11/2026.
