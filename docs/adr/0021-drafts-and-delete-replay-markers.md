# ADR-0021: Bản nháp theo thread và dấu xóa chống nhập lại

- **Status:** accepted
- **Date:** 2026-11-03
- **Deciders:** Yêu cầu bổ sung bản thử ARM64

## Context

Nội dung composer chưa gửi chưa có lưu trữ bền vững riêng theo thread. Xóa chỉ ở phía tôi xóa hàng SQLite nhưng import/sync trang cũ có thể đưa hàng đó trở lại. Học hướng xử lý từ PR upstream #62, triển khai riêng để giữ account namespace và worker của fork.

## Decision

Schema 8 bổ sung `composer_drafts(thread_id,text)` và `deleted_messages`, nằm trong CSDL riêng từng tài khoản. Migration từ schema 7 là bổ sung, giữ messages và coverage hợp lệ; migration 7 từ engine cũ vẫn chỉ chạy một lần.

Bản nháp chỉ gồm văn bản literal, tối đa 64 KiB, không lưu clipboard, file đính kèm hay đối tượng quote. Worker đọc/ghi SQLite; UI debounce 500 ms, lưu ngay khi đổi thread/đăng xuất. Kết quả load có owner/thread/generation; thao tác gõ hoặc xóa đã xảy ra thắng kết quả load muộn. Nháp rỗng xóa bản ghi. Thoát chủ động khi còn ghi nháp chờ ACK mới đóng, không thực hiện I/O trên UI. Crash/mất điện trước debounce có thể mất phần gõ cuối; không hứa bảo toàn từng phím.

Xóa nội dung vẫn là DELETE thật. Transaction giữ dấu chỉ gồm thread/message/client/sender identity và chiều gửi, không lưu nội dung hay raw JSON. SQLite BEFORE INSERT trigger và kiểm tra trong store từ chối replay; filter dùng trước hydrate/cache/UI/notification. ID server khác nhau không bị gộp chỉ vì client ID trùng; chỉ đối chiếu client khi một bên là ID tạm/phone-client, có chiều gửi và sender phù hợp. Dấu xóa có thể chặn tin cũ khi import; chưa có chức năng khôi phục dấu xóa.

Xóa hội thoại sau ACK máy chủ đánh dấu các hàng đã biết, xóa draft và dữ liệu hội thoại trong cùng transaction; tin mới với ID mới vẫn nhận. Không tuyên bố chặn được mọi tin cũ chưa từng có định danh ở máy này. **Dọn lịch sử cục bộ** vẫn cho phép sync tải lại như thông báo hiện có, không biến thao tác dọn cache thành xóa vĩnh viễn.

## Consequences

- Tin đã xóa không xuất hiện lại qua import/sync của các danh tính đã biết.
- Dấu định danh được giữ lâu dài; không giữ bản sao nội dung để khôi phục.
- Draft là dữ liệu riêng tư lưu giống lịch sử hiện có, không đưa vào log.
- Binary chỉ hiểu schema 7 trở xuống từ chối CSDL schema 8; rollback bằng bản sao lưu, không hạ user_version.

## Verification

Test SQLite thật trong RAM/tempdir kiểm tra rollback khi DELETE lỗi, chống insert từ writer cũ, import backup cũ, alias phone/server/local, cách ly thread/account, nháp literal và reopen. Test GUI kiểm tra load muộn, đổi tài khoản/thread, ACK cũ và đóng khi còn lưu nháp. Không dùng dữ liệu người dùng thật.
