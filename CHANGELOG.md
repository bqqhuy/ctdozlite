# Changelog

## v0.004-arm — Windows 11 ARM64 — 04/11/2026

Thay đổi so với [v0.003-arm](https://github.com/bqqhuy/ctdozlite/releases/tag/v0.003-arm).

- Menu chuột phải cả dòng tin; thanh cảm xúc và trả lời khi hover, giữ mô hình reaction của fork.
- Mention nhóm có UID thật, gợi ý thành viên và @All; giữ metadata trong bản nháp, trích dẫn và gửi lại.
- Thêm nút về tin nhắn mới nhất khi đang đọc lịch sử.
- Upload đọc thêm control trực tiếp/mảng và `fileUrl`; chờ xử lý tối đa 90 giây, báo lỗi thay vì gửi tệp thiếu URL.
- Không đổi schema hoặc luồng đồng bộ điện thoại.

## v0.003-arm — Windows 11 ARM64 — 04/11/2026

Thay đổi so với [v0.002-arm](https://github.com/bqqhuy/ctdozlite/releases/tag/v0.002-arm).

### Giao diện và thông báo

- Sửa theme sáng: không kế thừa chữ trắng của theme dark phẳng; đồng bộ màu
  selection, link, lỗi và cảnh báo theo giao diện đang chọn.
- GUI release không tạo cửa sổ Terminal phụ khi mở từ desktop. CLI chủ động dùng
  lại terminal cha.
- Popup thông báo hiển thị 10 giây; giữ khi rê chuột và khởi động lại thời gian
  đọc khi thay thông báo.
- Thêm dấu @ riêng cho mention chưa đọc, nhãn preview đúng tin và nhảy tới tin
  mention trong lịch sử cục bộ. Mở hội thoại không xóa dấu của mention chưa thấy.

### Ảnh, video, clipboard và tệp

- Video thêm thanh tiến độ, thời gian hiện tại/tổng, rê chuột xem mốc tua, click/kéo
  để tua; giữ trạng thái play/pause sau kéo, có mute và bố cục theo DPI.
- Gallery ảnh/video có điều hướng trước/sau từ SQLite, zoom/fit/1:1, lưu tệp và
  sao chép ảnh đang xem. Dùng thumbnail sẵn có khi bản xem lớn hơn đang tải.
- Thumbnail giữ tỷ lệ, bo góc và tránh phóng ảnh nhỏ. Hiển thị album khi có
  metadata nhóm ảnh rõ ràng; không gom các ảnh độc lập chỉ vì gửi gần nhau.
- Clipboard Excel hỗ trợ ảnh PNG/DIB và EMF dự phòng; preview trước khi gửi,
  phục hồi văn bản nếu đọc ảnh lỗi. Ctrl+Shift+V/menu dán chữ; áp dụng lựa chọn
  image/text/ask và ràng buộc kết quả theo tài khoản/hội thoại.
- Tệp đính kèm có Mở qua cache và Lưu thành riêng. Cache theo tài khoản và định
  danh tin/tệp, chống tải trùng, dọn bản cũ và bảo vệ tệp đang tải.

### Phiên bản và cập nhật

- Updater kiểm tra release fork `bqqhuy/ctdozlite`, hiểu nhãn `v0.NNN-arm` và tag
  semver cũ; giữ checksum và xác minh kiến trúc ARM64.
- Nhãn sản phẩm là `v0.003-arm`; phiên bản số Cargo/file là `0.0.4`, manifest
  `0.0.4.0`. Tên artifact theo nhãn sản phẩm; buildinfo ghi cả hai.
- Từ v0.002-arm cần tải/cài bản mới thủ công một lần vì updater cũ chưa đọc được
  nhãn tùy chỉnh.

Video vẫn tải xong trước khi phát và dùng cửa sổ MFPlay riêng; đóng player để
quay về gallery. Không đổi schema hoặc giao thức đồng bộ điện thoại/server.
Kiểm tra tự động không thay thế thử Excel, media và thiết bị ngoại vi trên máy
người dùng. Xem [checklist native](docs/V0_003_ARM.md).

## v0.002-arm — Windows 11 ARM64 — 04/11/2026

Tổng hợp thay đổi so với bản phát hành đầu tiên của upstream:
[ctdoteam1110/ctdozlite v0.0.1](https://github.com/ctdoteam1110/ctdozlite/releases/tag/v0.0.1),
commit `b2c862c63eccc661560a3e81dcb22b13238e4b4c`.

Bản này giữ nguyên hai EXE từ build đã kiểm thử tại commit
[`c886171`](https://github.com/bqqhuy/ctdozlite/commit/c886171),
[CI native Windows ARM64](https://github.com/bqqhuy/ctdozlite/actions/runs/41684142970).
`v0.002-arm` là nhãn release theo yêu cầu của maintainer; phiên bản nhúng trong EXE,
tên tệp và manifest build vẫn là `0.0.3`. Không đổi tên binary hoặc sửa metadata của
bản đã kiểm thử. Nhãn tùy chỉnh này chưa được bộ cập nhật hiện tại nhận dạng;
hãy tải portable hoặc installer trực tiếp từ release.

### Windows ARM64 và trải nghiệm desktop

- Hoàn thiện EXE native ARM64, renderer Direct3D 12 và phương án dự phòng; đóng gói
  portable cùng installer Windows. Liên kết CRT tĩnh, không cần cài riêng Electron,
  Node.js, FFmpeg hoặc VC++ Redistributable cho các luồng native đã tích hợp.
- Hoàn thiện thanh tiêu đề Windows, resize/Snap, bố cục thích ứng và cài đặt có cuộn.
- Đóng cửa sổ xuống tray; menu mở/thoát ứng dụng. Thông báo tin mới dùng badge taskbar
  và dấu báo tray, giảm nhấp nháy; có thông báo desktop và âm thanh.
- Sửa danh sách chat nhảy vị trí khi mở nhóm dài, giữ lịch sử hiển thị có giới hạn,
  giảm repaint và chỉ nạp ảnh/avatar gần vùng đang xem.
- Bảo vệ phiên bằng Windows DPAPI; tùy chọn ghi nhớ đăng nhập và tách dữ liệu,
  lịch sử, cache theo tài khoản. Hoàn thiện chọn thiết bị và hộp thoại tệp native.

### Đồng bộ và lưu lịch sử

- Bổ sung luồng lịch sử được điện thoại xác nhận: nhận metadata, tải container,
  giải mã và nhập SQLite; có tiến độ, hủy và log chẩn đoán đã giới hạn dữ liệu.
- Hoàn tất lượt theo đủ mọi `idx` tới gói mang `isLast`; xử lý gói lệch thứ tự,
  trùng gói và lỗi từng phần. Ghi checkpoint sau khi dữ liệu đã nhập xong.
- Tối đa bốn phiên song song, giãn nhịp mở phiên, mặc định 30 hội thoại/yêu cầu;
  xử lý trạng thái Idle, đánh thức, thử lại hữu hạn, mất kết nối và dọn phiên cũ.
- Cập nhật theo checkpoint và `lastTs`, bỏ qua hội thoại không đổi; có chế độ
  bảy ngày và quét lại toàn bộ. Hội thoại chưa ánh xạ được giữ riêng và có thử lại.
- Máy chủ: tải lịch sử nhóm/Cloud còn được cung cấp, tiếp tục cursor đang dở,
  ghi nhớ trạng thái hoàn tất và chỉ lấy phần mới ở lần sau. Hiển thị hội thoại
  bị server giới hạn, lưu kết quả giới hạn có thời hạn và cho phép hỏi lại.
- Khởi động không tự quét lại toàn bộ lịch sử. Lưu cursor HTTP/WebSocket theo
  tài khoản; không giới hạn cứng 64 trang khi cursor vẫn tiến. Cursor lặp có
  nghỉ/thử lại hữu hạn; tiến độ bền vững không vượt qua batch chưa ghi xong.
- Refresh chỉ ghi các tin mới/đổi dựa trên baseline thực sự đọc từ SQLite;
  giữ cập nhật metadata, tránh ghi hai lần, và không bỏ sót preview mới từ server.
- Nhập/xuất lịch sử SQLite/JSON tương thích; lưu bản nháp. Tin xóa trên máy có
  dấu định danh chống xuất hiện lại sau import/đồng bộ, không giữ nội dung đã xóa.

### Tin nhắn và tương tác

- Đối soát tin gửi bằng server/client ID; giữ client ID khi gửi lại, phân biệt
  đang gửi, đã gửi, đã nhận và đã xem. Không tự gửi lần hai khi kết quả chưa rõ.
- Hoàn thiện trả lời/trích dẫn và cuộn tới tin gốc đã nạp; không đoán ID còn thiếu.
- Hoàn thiện đọc rich text/RTF/Markdown, nội dung đa phương tiện và mốc ngày/giờ
  theo múi giờ hệ điều hành. Sửa Enter trong lúc bộ gõ đang composition.
- Thêm nhận/gửi trạng thái đang soạn với giới hạn tần suất và tùy chọn tắt;
  tiếp nhận biên nhận giao/đọc, giới hạn cập nhật đúng hội thoại.
- Hoàn thiện menu ghim, ẩn, đánh dấu đã đọc, mute và thao tác xóa; đồng bộ mute
  hai chiều. Giữ đúng tên nhóm thật bắt đầu bằng “Nhóm ”.
- Bổ sung danh sách thành viên, vai trò trưởng/phó nhóm, tìm kiếm, mention theo
  offset UTF-16; sửa avatar tròn, giữ tỷ lệ và bố cục avatar nhóm.
- Không hiển thị lịch sử gọi/nhỡ/không xác định trong chat; vẫn xử lý tín hiệu
  cuộc gọi để dừng chuông khi đã được nhận tại thiết bị khác.

### Ảnh, video, tệp và sticker

- Thumbnail và viewer trong ứng dụng; ẩn URL vận chuyển, cache có ngân sách,
  ưu tiên media đang xem, giới hạn kích thước và số tác vụ tải/giải mã.
- Nhận diện URL CDN mới, bản ghi cũ chỉ còn URL và đường tương thích JPEG/JXL.
  Phân loại lỗi HTTP, mạng, nội dung lỗi và giải mã; bỏ retry vô hạn.
- Phục hồi link qua `renewlink`: đọc các lớp phản hồi, biến thể URL, thumbnail
  video dạng chuỗi và trạng thái `async`; gộp yêu cầu và có deadline/hủy.
- Upload theo khối 1 MiB, tối đa hai tệp đang truyền; chờ `file_done`/`video_done`
  trước khi gửi tin. Thông báo upload được tách theo tài khoản và dọn khi hủy.
- Tùy chọn ảnh gốc hoặc bản HD tạo trên máy; quản lý, dọn và tái tạo bản sao HD
  khi gửi lại nếu ảnh gốc chưa thay đổi.
- Gửi video MP4/MOV/M4V theo luồng video, đọc metadata và tạo poster bằng Media
  Foundation trên Windows. Định dạng/luồng không phù hợp có đường gửi như tệp.
- Chuyển tiếp ảnh/video/tệp bằng sao chép trên server, có phương án tải lại và
  gửi lại ở bước chuẩn bị. Không fallback sang một lần gửi khác sau yêu cầu cuối.
- Chụp vùng màn hình trên Windows vào ô soạn, xem trước rồi mới gửi; kiểm tra
  tài khoản/hội thoại để tránh đính kèm nhầm sau khi chuyển cửa sổ chat.
- Tiếp nhận sticker, bộ GIF Ami, danh bạ hợp nhất và các cải tiến giao diện từ
  các bản upstream sau v0.0.1; port chọn lọc các tính năng PR #78.

### Audio/video, công cụ và kiểm chứng

- Tích hợp WASAPI/CPAL, Opus và Media Foundation/H.264 native; bổ sung luồng
  relay/SFU nhóm, trộn audio theo nguồn và giới hạn video từ xa.
- Bổ sung các công cụ tài khoản: proxy/profile, lịch gửi trên máy và cấu hình
  tự động hóa; tác vụ lên lịch cần ứng dụng đang chạy.
- CI Windows ARM64: 502 kiểm thử đạt, một codec test bổ sung đạt, kiểm tra PE,
  dependency/runtime, version resource, manifest và 21 smoke test giao diện DX12.
  Kiểm thử Linux của cùng mã nguồn: 496 đạt; clippy với warnings denied đạt.
- Cập nhật tài liệu giao thức, ADR, fixture và audit; giữ ghi nhận tác giả upstream.

### Giới hạn còn lại

- CI dùng GPU phần mềm; cần thử thêm GPU, mic/camera/Bluetooth, sleep/resume,
  cuộc gọi và sync iPhone trên thiết bị thật. Không cam kết mọi tài khoản có thể
  lấy đủ lịch sử máy chủ hoặc toàn bộ lịch sử điện thoại.
- Phục hồi URL CDN không đồng nghĩa đã hỗ trợ lấy dữ liệu zCloud. File hết hạn,
  đã bị xóa hoặc cần khóa khác có thể chưa mở được.
- CSDL cũ thiếu checkpoint đáng tin cậy có thể cần một lượt kiểm tra lịch sử;
  không coi số tin hiện có hoặc `import = 0` là bằng chứng đã đồng bộ đủ.
- EXE/installer chưa ký số. Yêu cầu Windows 11 ARM64 build 22000 trở lên;
  Windows N cần Media Feature Pack cho các luồng Media Foundation.
- Không tiếp nhận chức năng chống thu hồi của PR #78.

Cảm ơn upstream [ctdoteam1110/ctdozlite](https://github.com/ctdoteam1110/ctdozlite)
và các tác giả đóng góp. Bản fork tiếp nhận nhiều tính năng upstream và bổ sung,
kiểm chứng các thay đổi riêng cho Windows on Arm; không phải sản phẩm chính thức của Zalo.
