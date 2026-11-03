# ADR-0023: Đồng bộ điện thoại song song, theo pha và tiếp tục sau khi mất kết nối

- Status: accepted
- Date: 2026-11-03
- Supersedes: một phiên điện thoại tại một thời điểm (ADR-0017), dừng cả lần đồng bộ khi một lượt lỗi, timeout 5 phút cố định.
- Evidence: `re-notes/17-zalo-pc-sync-config-and-reconnect.md`.

## Context

Bản sửa hàng đợi control giúp đồng bộ chạy hết 19 lượt (khoảng 632.000 tin), nhưng còn ba vấn đề: iPhone có thể ngừng gửi giữa lượt (iOS tạm dừng app); WebSocket mất kết nối thì cả lần đồng bộ dừng và iPhone còn "Đang đồng bộ"; người dùng phải chờ hết mới thấy tin gần đây. Cấu hình mặc định của Zalo PC 26.11.10 không giới hạn khoảng thời gian, chia pha và chạy tối đa 4 phiên.

## Decision

- **Socket:** vòng đọc chuyển sự kiện cho giao diện qua hàng đợi sâu (4096) do một tác vụ riêng xả, để ping không bị chặn khi giao diện bận. Ghi log mã đóng kết nối và thời gian nghẽn.
- **Dọn phiên:** mỗi epoch mới đều gửi 591 lý do 3 cho các syncId trong journal. Journal lưu danh sách (tối đa 16), đọc được định dạng cũ.
- **Tiếp tục sau mất kết nối:** chờ epoch mới tối đa 30 giây, dọn phiên cũ, xin lại các lượt đang dở bằng phiên mới; tối đa 2 lần mỗi lần đồng bộ. Lượt danh sách hội thoại không tự tiếp tục.
- **Đánh thức (592):** nút "Đánh thức điện thoại"; tự gửi một lần sau 30 giây không có gói; tối thiểu 15 giây giữa hai lần. Máy chủ trả 0 thì cho thêm 180 giây.
- **Chờ:** xác nhận tối đa 600 giây; Idle liên tục 600 giây coi là đứng; 300 giây không có gói (ngoài thời gian Idle và thời gian ân hạn sau đánh thức) coi là đứng; mốc tổng 2 giờ tính từ khi xác nhận.
- **Lượt đứng:** xin lại một lần ở cuối hàng đợi, thất bại lần nữa thì bỏ qua, không ghi checkpoint; 2 lượt liên tiếp thất bại thì dừng và giữ dữ liệu.
- **Song song và pha:** tối đa 4 phiên tin nhắn, cách nhau 600 ms; 10 hội thoại mới nhất chưa có coverage được lấy trước 100 tin (không ghi checkpoint), sau đó các lượt đầy đủ 30 hội thoại. Tiến độ gộp một dòng.
- **Khoảng thời gian:** mặc định toàn bộ như Zalo; tùy chọn 1 năm / 90 / 30 ngày. Coverage chạm tới mốc thì chỉ lấy phần mới.
- Gói trùng idx chỉ khác URL được bỏ qua; khác loại gói vẫn là xung đột. Checkpoint chỉ cho hội thoại đã ánh xạ. Lỗi mạng khi tra ID người gửi được thử lại ở gói sau.
- Hàng đợi kéo đã biết (510/511/513–518/603/604) xuất hiện trong trang hợp lệ nhưng không có trong auth được đăng ký và kéo tiếp từ đúng cursor.

## Verification / rollback

Test tự động: chuyển tiếp sự kiện giữ thứ tự, gói trùng ký lại, gợi ý đứng/Idle/hoạt động lại, journal định dạng cũ, mốc khoảng thời gian, gộp tiến độ, giới hạn 100 tin cho pha ưu tiên, phát hiện hàng đợi. Cần thử iPhone thật: kết nối lại giữa lượt, đánh thức khi đứng, 4 phiên song song, và iPhone thoát trạng thái đồng bộ sau mất kết nối.

Không đổi schema. Rollback về ADR-0017 bằng cách đặt 1 phiên và bỏ tiếp tục; mã cũ không đọc được journal dạng danh sách và sẽ từ chối đồng bộ, nên trước khi chạy bản cũ cần xóa tệp `*.sync-session.json` của tài khoản (chỉ chứa syncId; điện thoại tự hết hạn các phiên đó).
