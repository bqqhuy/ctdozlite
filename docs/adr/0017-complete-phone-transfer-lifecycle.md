# ADR-0017: Hoàn tất và vòng đời phiên đồng bộ điện thoại

- Status: accepted
- Date: 2026-11-03
- Supersedes: ADR-0014 scoped completion, round-level descriptor barrier and overlapping rounds; ADR-0010 lifecycle where incompatible.

## Context

Người dùng báo iPhone đã xác nhận nhưng đứng ở lượt 1/30 hội thoại. Đối chiếu worker từ Zalo PC 26.11.10 cho thấy `checkRequestfulfilled` dùng tập idx và totalBatch=idx(isLast)+1; scopes không phải dấu kết thúc riêng từng partition. Fixture của engine cũ nhận idx0 và idx1 cuối nhưng vẫn chờ partition không nằm trong gói cuối. Bằng chứng: `re-notes/15-phone-lifecycle-and-renewlink.md`.

## Decision

Một phiên điện thoại chủ động tại một thời điểm là lựa chọn bảo thủ của fork, không phải giới hạn giao thức. Nhận descriptor liên tục, tải hai gói song song vào staging riêng, hàng đợi kết quả hai phần tử. Descriptor tối đa 2048, mỗi container giữ ngân sách 64 MiB. Tối đa tám file message chờ khóa metadata; vượt giới hạn thì dừng rõ ràng thay vì dồn RAM hoặc deadlock. Descriptor message đợi metadata giải mã thành công trước khi mở download, tránh file chờ làm nghẽn metadata. Metadata giải mã trước message phụ thuộc; import qua một writer SQLite. Gói tới trước/sau isLast đều được theo dõi; chỉ kết thúc nhận khi có mọi idx 0..cuối, chống trùng toàn phiên và từ chối idx xung đột/vượt giới hạn.

Dispose sau xử lý tải/giải mã/import, với lý do 0 hủy, 1 thành công, 2 lỗi và 3 phiên cũ. Journal account-scoped chỉ lưu syncId, không khóa hay URL; startup sau khi socket sẵn sàng và trước sync chủ động đều dọn journal. Không replay yêu cầu sync qua WS epoch mới. Dispose thất bại giữ journal cho lần dọn sau.

Idle tạm dừng bộ đếm chờ hoạt động; Active tiếp tục. Chờ xác nhận tối đa 180 giây hoạt động; chờ gói tiếp 120 giây hoạt động, toàn phiên tối đa 1800 giây. Đây là mặc định bảo thủ của fork, không tuyên bố là cấu hình server chính thức. Trạng thái 6–10 kết thúc có thông báo riêng. Tải container dùng read-idle timeout 30 giây và deadline 1800 giây; GET bị gián đoạn được thử lại tối đa một lần, không khởi tạo lại điện thoại. Rate limit được lưu cooldown.

Gói message báo lỗi với scopes xác định không được nhập/ghi coverage; các phần độc lập vẫn nhập. Metadata lỗi hoặc lỗi không xác định phạm vi dừng fail-closed. Unmapped dữ liệu khiến cả lượt không tiến coverage. Schema 7 vô hiệu hóa một lần phone_sync_ranges của engine cũ, giữ messages, history_sync, lịch gửi và cấu hình. Dữ liệu nhận lại được nhập idempotent theo INV-4.

Giữ tối đa 30 hội thoại/request và chỉ split sau lỗi giới hạn rõ ràng. Cận trên hiện giữ snapshot thời gian hiện tại, chưa cắt theo lastTs khi chưa có nghiệm thu đầy đủ timestamp. Không dùng đường wake-up/polling upstream thay thế phone container; RSA/ZDB4 để nghiên cứu sau, không tự fallback.

## Verification and rollback

Fixture: isLast tới trước, gói tới cuối có isLast=false, thiếu/trùng/idx bất thường, foreign session, Idle/Active, lỗi trạng thái, metadata/message lỗi. Migration chứng minh giữ tin, chỉ reset coverage một lần. Cần thử iPhone thật với tin chỉ còn trên điện thoại và xem điện thoại thoát trạng thái đồng bộ.

DB schema 7 không được mở bằng binary chỉ hiểu schema 6; rollback dùng binary tương thích schema 7, không hạ user_version. Không xóa tin để rollback.
