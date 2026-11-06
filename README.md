# ctdozlite · Windows 11 ARM64

Ứng dụng Zalo cộng đồng viết bằng **Rust**, dành riêng cho **Windows 11 trên ARM64**. Giao diện dùng egui và Direct3D 12; audio/video sử dụng các thành phần native của Windows.

Dự án này là fork của **[ctdoteam1110/ctdozlite](https://github.com/ctdoteam1110/ctdozlite)**. Nền tảng mã nguồn, nghiên cứu giao thức và nhiều tính năng bắt nguồn từ upstream cùng các tác giả đóng góp. Fork do [bqqhuy](https://github.com/bqqhuy/ctdozlite) duy trì để hoàn thiện trải nghiệm Windows on Arm.

[GitHub Actions](https://github.com/bqqhuy/ctdozlite/actions/workflows/windows-arm64.yml) · [Releases](https://github.com/bqqhuy/ctdozlite/releases) · [Hướng dẫn Windows ARM64](docs/WINDOWS_ARM64.md) · [Audit kỹ thuật](docs/ARM64_IMPLEMENTATION.md)

## Mục tiêu của bản fork

Chạy ứng dụng ARM64 native trên máy Windows 11 dùng Snapdragon, với giao diện desktop, khay hệ thống, lịch sử theo tài khoản và media trong cùng tiến trình. Không cần cài Node.js, Electron, FFmpeg hoặc VC++ Redistributable riêng.

Thiết bị đích được người dùng cung cấp: **Huawei MateBook E Go Gen 3 — Snapdragon 8cx Gen 3**. Kiểm thử CI hiện chưa thay thế được kiểm chứng GPU, camera, microphone và cuộc gọi trên thiết bị này.

## Tải và cài đặt

**Yêu cầu:** Windows 11 ARM64, build 22000 trở lên. Windows N cần **Media Feature Pack** để dùng camera/video.

- **Bản phát hành:** tải từ [Releases của fork](https://github.com/bqqhuy/ctdozlite/releases), đọc ghi chú của đúng phiên bản.
- **Bản thử nghiệm:** vào [Actions](https://github.com/bqqhuy/ctdozlite/actions/workflows/windows-arm64.yml), chọn lần chạy thành công của nhánh/commit cần thử, tải artifact **`ctdozlite-windows-arm64`**.

Bản **v0.008-arm** bổ sung hộp người lạ/privacy, chat ẩn PIN, nhãn/archive, hồ sơ và
quản trị nhóm; siết vòng đời cuộc gọi đa thiết bị và tùy chỉnh giao diện. Xem
[changelog so với v0.007-arm](CHANGELOG.md), gồm giới hạn và các ca cần thử thiết bị thật.
Nhãn sản phẩm `v0.008-arm`, phiên bản số Windows `0.0.9`.

| Tệp trong gói | Cách dùng |
| :--- | :--- |
| `ctdozlite-<version>-setup-windows-arm64.exe` | Cài ứng dụng bằng trình hướng dẫn. |
| `ctdozlite-<version>-portable-windows-arm64.exe` | Chạy trực tiếp. |
| `SHA256SUMS-windows-arm64.txt` | Đối chiếu SHA-256 của các tệp đi kèm. |
| `build-info-windows-arm64.json` | Xem commit, target, renderer và thông tin build. |

Có thể kiểm tra checksum trong PowerShell:

```powershell
Get-FileHash .\ctdozlite-<version>-setup-windows-arm64.exe -Algorithm SHA256
```

Thay `<version>` bằng phiên bản đã tải và so sánh với tệp checksum cùng gói. Ứng dụng cài bên trong là **PE ARM64**; trình cài Inno Setup dùng bootstrap x86. Bộ cài hiện **chưa ký số**.

## Bắt đầu sử dụng

1. Mở ứng dụng và đăng nhập bằng mã QR.
2. Cho phép ứng dụng desktop truy cập camera/microphone trong **Windows Settings → Privacy & security** nếu cần gọi.
3. Chọn thiết bị trong phần Cài đặt của ứng dụng.
4. Chờ cập nhật hội thoại; dùng mục đồng bộ để theo dõi việc tải lịch sử nhóm và Cloud.

**Đóng cửa sổ:** bấm **X** hoặc **Alt+F4** để ẩn xuống tray. Bấm chuột trái hoặc phải vào biểu tượng tray để mở menu **Mở Apps** / **Thoát Apps**. Chọn **Thoát Apps** để kết thúc ứng dụng.

## Các chức năng hiện có

| Phần | Hành vi |
| :--- | :--- |
| Giao diện | Renderer DX12, thanh tiêu đề Windows, kéo/đổi kích thước/Snap; Settings có cuộn và bố cục thích ứng. |
| Tin nhắn | Hội thoại cá nhân/nhóm, gửi tin, ảnh/tệp/sticker; lưu lịch sử bằng SQLite theo tài khoản. |
| Đồng bộ | Cập nhật hội thoại/tin gần đây sau đăng nhập; tải lịch sử nhóm/Cloud theo trang. Thanh trạng thái và nút dừng hiển thị ngay trên GUI chính. |
| Tên nhóm | Tải metadata nhóm và giữ tên đã lưu khi cập nhật gặp lỗi. |
| Menu hội thoại | Ghim, tắt/bật thông báo, ẩn/hiện, đánh dấu đã đọc và xóa lịch sử có xác nhận. Các lựa chọn này áp dụng **trên máy này**. |
| Phiên đăng nhập | Bảo vệ bằng Windows DPAPI, tùy chọn ghi nhớ đăng nhập, tách dữ liệu từng tài khoản. |
| Audio/video | WASAPI/CPAL, Opus và Media Foundation/H.264 native; đã tích hợp relay/SFU nhóm, trộn audio theo nguồn và tối đa hai video từ xa. |
| Tài nguyên | Giới hạn lịch sử giữ trong RAM, cache ảnh/texture và hàng đợi preview; lịch sử đầy đủ đã nhận vẫn ở SQLite. |

Hội thoại đã ẩn có thể mở lại qua **Hiện cả hội thoại đã ẩn**. Chức năng này không phải khóa PIN. Xóa lịch sử chỉ tác động bản lưu trên máy. Tin đã xóa có dấu định danh để chống xuất hiện lại sau import/đồng bộ; không lưu nội dung tin trong dấu xóa.

Đối soát tin gửi dùng server/client ID thay vì nội dung/thời gian, tránh gộp nhầm các tin giống nhau. Có thể bấm khối trích dẫn để cuộn tới tin gốc **đã nạp** trong hội thoại; tin chưa nạp hoặc ID không duy nhất sẽ được báo rõ. Xem [ADR-0006](docs/adr/0006-exact-message-identity-and-quote-navigation.md).

Ảnh nhận được hiển thị thumbnail và chú thích (nếu có), ẩn URL vận chuyển. Bấm ảnh để mở trình xem ảnh trong ứng dụng. Giờ tin mới, tin đồng bộ và mốc ngày dùng chung múi giờ Windows.

Đồng bộ điện thoại lấy danh sách hội thoại rồi xin phần còn thiếu; hội thoại không đổi được bỏ qua khi có checkpoint hoàn tất. Mặc định tối đa 30 hội thoại/yêu cầu và bốn phiên song song, có giãn nhịp mở phiên. Phiên đứng được xin lại có giới hạn; mất kết nối được phục hồi và dọn phiên cũ. Chỉ ghi checkpoint sau khi nhận đủ gói, giải mã và nhập xong. **Chỉ cập nhật tin gần đây** giới hạn phạm vi bảy ngày; **Quét lại toàn bộ lịch sử điện thoại** đặt lại tiến độ, giữ tin đã lưu. Vẫn cần điện thoại chấp thuận. Khi mở app, chỉ cập nhật metadata và hàng đợi tin mới; tải lịch sử máy chủ là thao tác chủ động.

Bản cũ chưa ghi checkpoint nên không thể suy ra đợt tải đã hoàn tất từ số tin hiện có. Với dữ liệu từ bản cũ, dùng chế độ cập nhật gần đây nếu chưa muốn tải lại phần lịch sử cũ. Thứ tự gói do điện thoại quyết định, không bảo đảm từng tin đến đúng thứ tự mới → cũ. Mỗi yêu cầu có thể nhận nhiều gói dữ liệu từ điện thoại; nhận thêm gói không phát sinh yêu cầu sync mới. Khoảng tiếp tục có chồng lấn 2 phút để tránh bỏ sót tin ở ranh giới. Xem [ADR-0017](docs/adr/0017-complete-phone-transfer-lifecycle.md).

Cài đặt → Đồng bộ có **Nhập / Xuất lịch sử** (SQLite/JSON tương thích; không phải mọi backup Zalo PC). Xóa hội thoại qua máy chủ là mục riêng với xóa lịch sử cục bộ và chỉ cập nhật local sau xác nhận. Media phân loại lỗi, retry hữu hạn, có phục hồi URL và nhánh JPEG thay cho JXL chưa giải mã được; chưa cam kết truy xuất zCloud. Xem [ADR-0018](docs/adr/0018-finite-media-recovery.md) và [ADR-0019](docs/adr/0019-history-tools-and-arm64-updater.md).

## Trạng thái kiểm chứng và giới hạn

Bản build tại [`c886171`](https://github.com/bqqhuy/ctdozlite/commit/c886171) đã qua [CI native Windows ARM64](https://github.com/bqqhuy/ctdozlite/actions/runs/41684142970): **502 kiểm thử đạt**, thêm một codec test với worker công khai, kiểm tra PE/metadata và **21 smoke test giao diện DX12**. CI dùng GPU phần mềm Microsoft Basic Render Driver. Kết quả này chưa thay thế thử nghiệm trên thiết bị ARM64 thật.

Những điểm cần biết trước khi dùng:

- Upload đọc theo khối **1 MiB**, tối đa **hai tệp đang truyền**, các tệp còn lại chờ; hộp chọn Windows hỗ trợ nhiều tệp. Không đặt giới hạn MB/tệp ở ứng dụng; máy chủ vẫn có thể áp dụng giới hạn riêng. Giới hạn giải mã preview ảnh 16 MiB vẫn giữ.
- Chuyển tiếp hiện hỗ trợ **tin chữ và sticker**. Chuyển tiếp ảnh/tệp/video bị khóa kèm hướng dẫn tải và gửi lại tệp gốc; ứng dụng không gửi nhãn chữ rồi báo đã chuyển tiếp media.
- Tin gửi lỗi giữ trong hội thoại cùng cảnh báo và nút **Gửi lại**; nút biến mất sau xác nhận thành công. Popup Windows hoạt động khi minimize hoặc ẩn tray, theo tùy chọn thông báo/âm thanh và tắt tiếng hội thoại. Xem [ADR-0009](docs/adr/0009-outgoing-retry-streaming-and-windows-popup.md).
- Nếu máy chủ chưa xác nhận gửi/thu hồi, ứng dụng báo chưa rõ kết quả và không tự gửi lại thành tin thường. Kiểm tra hội thoại trước khi thử lại. Trên Windows, dấu chưa đọc ở tray được cập nhật cả khi cửa sổ ẩn; âm báo dùng tùy chọn thông báo hiện có. Xem [ADR-0008](docs/adr/0008-fail-closed-mutations-and-background-unread.md).
- **Cuộc gọi còn cần kiểm chứng thực tế:** codec và gói giao thức đã có kiểm thử; gọi đi/đến, gọi nhóm, NAT, Bluetooth và camera trên MateBook chưa được xác nhận đầy đủ với client Zalo chính thức.
- Đã bổ sung **Settings → Đồng bộ → Đồng bộ từ điện thoại**: cần mở Zalo di động và xác nhận. Tiến độ và nút dừng hiện ở cửa sổ chính. Người dùng đã xác nhận luồng đồng bộ ban đầu hoạt động với điện thoại thật; cơ chế ưu tiên khoảng thời gian và tiếp tục mới vẫn cần kiểm chứng trên thiết bị; chỉ lấy được dữ liệu điện thoại còn giữ, không bảo đảm media cũ còn hạn. Xem [ADR-0005](docs/adr/0005-phone-approved-history-sync.md).
- Chưa có nguồn presence đáng tin cậy để hiển thị trạng thái online của mọi liên hệ.
- Một số Settings kế thừa chưa có hành vi đầy đủ. Cài đặt → Tiện ích có kiểm tra/tải cập nhật thủ công từ release fork, kiểm SHA-256 và payload ARM64 trước khi cho mở bộ cài. Chưa bật Social Feed hoặc REST/MCP.
- Database hiện dùng **schema 8**: thêm bản nháp theo hội thoại và dấu định danh chống nhập lại tin đã xóa; giữ tin nhắn/cấu hình/coverage hợp lệ từ schema 7. Khi nâng từ engine cũ, migration 7 vẫn vô hiệu hóa coverage điện thoại nghi sai một lần. Binary chỉ hiểu schema 7 trở xuống sẽ từ chối mở; sao lưu trước khi thử. Xem [ADR-0021](docs/adr/0021-drafts-and-delete-replay-markers.md).
- Composer lưu bản nháp văn bản theo tài khoản/hội thoại qua worker sau 500 ms; kết quả đọc muộn không ghi đè nội dung đang gõ. Chưa lưu đính kèm/quote chưa gửi.
- HTTP/WS tiếp tục khi cursor còn tiến, không cắt sau 64 trang. Queue WS do server cấp có timer đọc lại hữu hạn khi thiếu/lặp cursor hoặc hết thời gian chờ; lịch hủy theo kết nối. Không tăng cursor giả hay tự chạy lại sync điện thoại. Xem [ADR-0022](docs/adr/0022-resumable-control-queue-drain.md).
- Sync điện thoại hiện rõ số mô tả gói, idx lớn nhất, lỗ hổng và chờ dấu cuối; nhập xong chuyển sang chờ gói tiếp. Log `diagnostics/sync-media.jsonl` ghi từng stage và quyết định queue, không chứa URL/khóa/nội dung tin. Mốc chờ gói thử nghiệm là 5 phút hoạt động; điều kiện hoàn tất vẫn nhận đủ idx rồi nhập thành công, schema giữ nguyên 8.

Đây là tích hợp có chọn lọc từ upstream, chưa phải bản sao đầy đủ mọi tính năng của upstream mới nhất. Chi tiết có trong [báo cáo đối chiếu commit](docs/UPSTREAM_REVIEW_2026-11-02.md) và [audit Windows ARM64](docs/ARM64_IMPLEMENTATION.md).

## Build trên Windows 11 ARM64

Chuẩn bị Visual Studio 2022 Build Tools với **ARM64 MSVC và Windows SDK**, Rust ARM64, CMake, LLVM và Inno Setup 6.7.1 trở lên. Phiên bản Rust được ghim trong [rust-toolchain.toml](rust-toolchain.toml).

```powershell
git clone https://github.com/bqqhuy/ctdozlite.git
cd ctdozlite
git switch fix/windows-arm64-product
rustup set default-host aarch64-pc-windows-msvc
.\scripts\build-windows-arm64.ps1
```

Script chạy kiểm thử, build release, kiểm tra PE ARM64 và metadata, thử renderer/tray rồi tạo gói trong `dist/`. Xem [hướng dẫn đầy đủ](docs/WINDOWS_ARM64.md) nếu cần cấu hình công cụ hoặc chẩn đoán lỗi build.

## Báo lỗi và bảo vệ dữ liệu

Tạo issue tại [repo của fork](https://github.com/bqqhuy/ctdozlite/issues), kèm commit/phiên bản, model máy, build Windows, cách tái hiện và tên/driver thiết bị liên quan. Với lỗi camera hoặc cuộc gọi, ghi rõ gọi đi hay gọi đến và loại mạng đang dùng.

Không đưa session, cookie, IMEI, database hoặc log chứa nội dung chat lên issue công khai. DPAPI bảo vệ phiên đăng nhập; **database và cache chat chưa được mã hóa**. Đọc [SESSION_SECURITY.md](SESSION_SECURITY.md) trước khi chia sẻ thông tin chẩn đoán.

## Credit và giấy phép

**Cảm ơn dự án gốc [ctdoteam1110/ctdozlite](https://github.com/ctdoteam1110/ctdozlite) và các tác giả upstream.** Bản fork này phát triển tiếp trên công sức nghiên cứu giao thức, xây dựng client và cải thiện ứng dụng của cộng đồng upstream.

- **ctdoteam1110 / thanhdo1110:** ghi nhận tác giả và bản quyền gốc theo [LICENSE-MIT](LICENSE-MIT); [thanhdo1110](https://github.com/thanhdo1110) có đóng góp về GUI và tái cấu trúc giao diện được upstream ghi nhận.
- **[Thuong Tin / @thuongtin](https://github.com/thuongtin):** đóng góp upstream về thời gian, lưu phiên và các nền tảng xử lý message state, media/cache, updater. Phạm vi tích hợp vào fork được mô tả riêng trong audit.
- **[Các tác giả và người đóng góp upstream](https://github.com/ctdoteam1110/ctdozlite/graphs/contributors):** nghiên cứu, mã nguồn, báo lỗi, kiểm thử và đề xuất tính năng. Lịch sử Git và các ghi nhận bản quyền được giữ nguyên.
- **[bqqhuy](https://github.com/bqqhuy/ctdozlite):** duy trì fork chuyên Windows 11 ARM64 và các điều chỉnh dành cho Windows on Arm.

Mã nguồn được phân phối theo **MIT hoặc Apache-2.0**: [LICENSE-MIT](LICENSE-MIT) · [LICENSE-APACHE](LICENSE-APACHE). Giấy phép và ghi nhận thư viện phụ thuộc nằm trong [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).

Zalo và các nhãn hiệu liên quan thuộc VNG. Đây là ứng dụng cộng đồng không chính thức, độc lập và không được Zalo/VNG bảo trợ.

### Tiện ích mới theo tài khoản

Mở **Cài đặt → Tiện ích → Tìm lịch sử, nhập dữ liệu, theme/sticker, hẹn giờ & tự trả lời**.

- Tìm toàn bộ SQLite (không dấu, tối đa 50 kết quả mới nhất), bấm kết quả để mở ngữ cảnh cục bộ.
- Nhập SQLite/JSON tương thích từ tập tin/thư mục, nguồn chỉ đọc và cần xác nhận đúng tài khoản. JSON tối đa 32 MiB; lịch sử lớn dùng SQLite. Chưa hỗ trợ JSONL hoặc giải mã backup Zalo PC. Thư mục nhận các DB và `threads_cache.json`/`export.json`, bỏ qua cấu hình/phiên đăng nhập.
- Nạp/xuất theme JSON và danh sách sticker theo ID Zalo; cấu hình lưu riêng từng tài khoản.
- Hẹn giờ tương đối hoặc `YYYY-MM-DD HH:MM` theo giờ máy, sửa tin chưa từng gửi, hủy và gửi lại thủ công. App cần đang chạy/đăng nhập; tin quá giờ sẽ gửi khi mở/đăng nhập lại, nhưng không tự gửi lại tin đã bị gián đoạn. Lịch gửi tạm dừng sau lỗi và chỉ tiếp tục khi người dùng yêu cầu.
- Quy tắc tự trả lời mặc định tắt, có khớp chính xác/chứa/tiền tố và phạm vi cá nhân/nhóm. Chỉ áp dụng tin mới sau lúc bật, chống trùng bền vững và tối đa 5 trả lời/phút. Thay cấu hình hủy các trả lời tự động còn chờ.

Các phần này tiếp nhận có chọn lọc từ upstream `8e16464`, giữ định danh tin, bảo vệ tài khoản, native Windows ARM64 và đường gửi có xác nhận của fork. REST/MCP và updater tự động chưa bật.
