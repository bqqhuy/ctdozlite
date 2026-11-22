# Sửa sidebar và clipboard sau phản hồi native

Ngày: 22/11/2026. Không xác nhận hoàn tất P5 từ compile/test.

## Bằng chứng Zalo PC

E2: `compact-app-pc.0cd2e653e67efaa4bf4b.js` trích từ bản cài 26.11.12, SHA-256 `78801014154A86BADE096EB916E541223A5F553F888882AA0EF3A218A44D6C08`. Offset dưới đây là chỉ số chuỗi .NET, không phải offset byte UTF-8.

- Khoảng 5025825: `getConfigCopyExcelMode`; chế độ 1 dùng `_uploadDragPhoto` với `sendFormatExcel`, chế độ 2 chèn text; chưa lưu lựa chọn và popup bật thì mở `COPY_EXCEL`. Không thể kết luận mọi tài khoản luôn dán Excel thành ảnh.
- Khoảng 1475675: hộp chọn định dạng Excel có ảnh xem trước và lựa chọn lưu mode.
- Khoảng 2647391: paste phân biệt text/html, text/plain, file và mixed.
- Khoảng 4926565: `_checkLimitImagePrevew` kiểm tra tổng preview hiện có cộng số mục mới với `maxNumberOfFile`. Giá trị 50 được người dùng xác minh trực tiếp.

## Root cause sidebar

Egui 0.29 SidePanel clip paint theo planned rectangle nhưng reserve/store chiều rộng theo final content rectangle. Content search/banner có thể nở rộng, khiến phần reserve lớn hơn phần đã paint. Patch màu banner và window_margin trước đó không giải quyết nguyên nhân này; bỏ nhận định chúng đã sửa vạch đen.

Patch mới cô lập nội dung trong child UI theo bounds của panel; parent không nhận content min-width bị nở. Search budget tính cả frame/icon/clear và bỏ minimum width ép tràn. `screen_rect.width()` là logical points, không chia pixels_per_point lần nữa.

Regression kiểm tra actual panel response rect trước/sau search dài, sync đang chạy và clear tại DPI 1/1.5/2: passed. Native binary mới chưa xác nhận.

## Clipboard

CF_HDROP được đọc ở worker và sao chép toàn bộ danh sách đường dẫn; không giải mã file/video đầu tiên thành ảnh. Kết quả đi qua owner/runtime/thread guard rồi vào attachment draft, không tự gửi. Quá số lượng hoặc vượt phần còn lại của tổng 50 thì từ chối cả batch và toast. Bitmap/DIB/EMF giữ flow ảnh; chế độ Excel image/text/ask được giữ.

Nút + trong preview phải gọi PickAttachmentDraft; toolbar picker vẫn dùng intent gửi ngay.

## Crash và giới hạn

Kiểm tra sau sửa: sidebar actual panel rect 1 passed; file-drop/draft 9 passed; picker 51-file production guard 1 passed; copied-files event boundary 1 passed. Cargo check --tests --locked -j 1 qua. Chưa xác nhận native binary mới.

Clippy --all-targets --locked -j 1 -- -D warnings, cargo fmt --all -- --check và git diff --check: exit 0 sau delta.

Đã tìm được `last-panic.txt` của app trong data root, ghi `source=wgpu_core.rs:2314` sau `clipboard-copy > clipboard-decode > clipboard-preview`. Vị trí này là GPU Queue::submit; không phải bằng chứng file tạm mất hoặc server error 204. Bỏ kiểm tra is_file trên UI thread vì không chữa crash và vi phạm no blocking UI. Chưa tuyên bố crash được sửa.

Cần native test: search/sync không đổi panel hoặc tạo vạch đen; copy nhiều file/video/ảnh từ Explorer vào preview; Excel image/text/ask; 49+1/49+2 và batch 51; huỷ/đổi thread; clipboard send không văng. Server 204, native voice upload và player inline vẫn chưa hoàn tất.
