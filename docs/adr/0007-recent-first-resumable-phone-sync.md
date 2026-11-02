# ADR-0007: Đồng bộ điện thoại ưu tiên gần đây và tiếp tục theo khoảng đã nhận

- **Status:** accepted
- **Date:** 2026-11-02
- **Deciders:** Maintainer request; implementation review

## Context

Người dùng xác nhận đồng bộ điện thoại đã hoạt động nhưng mất nhiều thời gian (ảnh báo 424.261 tin mới ở đợt 27/35). Luồng ADR-0005 yêu cầu từ 0 mỗi lần và sắp partition theo ID, không có bằng chứng về khoảng đã nhận đầy đủ. Số tin/min/max timestamp không chứng minh một khoảng đã hoàn tất. `re-notes/11_phone_sync.md` ghi giao thức có `partition/from/to`; tham chiếu công khai dùng `win.from` và thời điểm hiện tại. Không thay cmd/cipher/khóa/cơ chế chấp thuận. ADR này bổ sung ADR-0005 và thay lựa chọn luôn tải từ 0.

## Decision

Sắp hội thoại theo `lastTs` giảm dần, nhóm/cá nhân chung một thứ tự; tối đa 30 partition/đợt. Với partition mới, ưu tiên 7 ngày quanh hoạt động cuối rồi mở rộng 90 ngày và phần còn lại. Khoảng cuối kết thúc tại thời điểm bắt đầu lượt. Điện thoại vẫn quyết định thứ tự gói/tin trong mỗi khoảng. Chế độ chỉ cập nhật gần đây bỏ qua hai lượt backfill; quét lại xóa tiến độ nhưng giữ tin.

Schema 5 bổ sung `phone_sync_ranges` trong database từng tài khoản, khóa partition và kiểm tra chủ sở hữu bằng thread ID thật. Chỉ ghi coverage sau khi mọi gói của đợt đã xác thực/giải mã/nhập thành công. Khoảng nối tiếp phải giao nhau; không suy đoán qua khoảng trống. Tin được commit trước checkpoint: lỗi/dừng/crash tải lại khoảng dang dở và đối soát ID. Tin không nhận diện hội thoại vẫn giữ riêng, không lưu coverage bền vững để lần sau thử lại. Xóa lịch sử hội thoại cũng xóa checkpoint liên quan.

Lần tiếp theo yêu cầu `[to_đã_lưu − 2 phút, hiện_tại]`, giới hạn trong khoảng đã có, rồi backfill phần cũ chưa hoàn tất. Khoảng chồng lấn giảm rủi ro tin tới trễ nhưng không bảo đảm tìm lại mọi tin được thêm muộn vào vùng cũ. Quét lại toàn bộ là cách yêu cầu lại vùng đã đánh dấu. Điện thoại vẫn phải chấp thuận; đây không phải đồng bộ nền tự động. Bản cũ không có checkpoint nên cần tạo baseline mới, hoặc dùng chế độ gần đây để tránh tải lại lịch sử sâu ngay.

Giảm chi phí nhập bằng prepared statement cache, bỏ đối soát lại chính server ID, dùng chung bản đồ ID bất biến và nhớ ID chưa phân giải trong cùng lượt. Không thay quy tắc gộp tin/recall/local flags. Benchmark synthetic 5.000 tin × 5 lần trên SQLite RAM (debug) giảm trung bình ~825 xuống ~545 ms; không suy diễn thành tốc độ truyền điện thoại.

## Verification

Kiểm thử thứ tự, khoảng/overlap/resume, không gộp khoảng rời, transaction rollback, giữ tin khi reset, account isolation/reopen và serialization `from/to`. Bộ test hiện có tiếp tục kiểm tra danh tính tin, recall, flags và codec chính thức. Không dùng tài khoản hay điện thoại thật trong CI. Cần kiểm tra thủ công: gần đây xuất hiện trước; dừng/mở lại/chấp thuận tiếp tục; lần sau chỉ yêu cầu delta; so sánh số tin theo khoảng với điện thoại. Nếu điện thoại không tuân thủ range, kiểm tra metadata trước khi coi checkpoint là bằng chứng đầy đủ.

## Consequences / Rollback

Lần tải đủ đầu tiên có thể nhiều đợt hơn vì ba lượt ưu tiên; thời gian toàn bộ còn phụ thuộc điện thoại/mạng/giải mã. Ưu tiên dữ liệu hữu ích sớm và giảm tải lặp, không cam kết tốc độ tổng thể. Checkpoint không chứa khóa/cookie; không log nội dung/ID thực.

Migration 4 → 5 chỉ thêm bảng, giữ nguyên messages và raw source. Bản schema 4 từ chối database 5; rollback dùng mã tương thích schema 5 hoặc bản sao lưu trước nâng cấp. Không hạ `user_version`, không xóa database. Có thể khôi phục thuật toán from-0 mà vẫn giữ schema 5, bỏ qua bảng checkpoint.

## Alternatives Considered

- Chỉ sắp ID theo tên: không thể hiện độ mới.
- Suy checkpoint từ min/max timestamp hoặc số tin: có thể bỏ qua khoảng chưa tải.
- Tự đồng bộ không hỏi điện thoại: trái cơ chế chấp thuận hiện có.
- Tải song song nhiều round: chưa có bằng chứng tương thích sessionRecord/control và có thể tăng áp lực lên điện thoại.
