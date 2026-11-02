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

Các thay đổi mới đang được tập hợp tại [PR #2](https://github.com/bqqhuy/ctdozlite/pull/2). Bản trong Releases có thể chưa chứa các thay đổi trên nhánh này.

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

Hội thoại đã ẩn có thể mở lại qua **Hiện cả hội thoại đã ẩn**. Chức năng này không phải khóa PIN. Xóa lịch sử chỉ xóa bản lưu trên máy; đồng bộ sau đó có thể tải lại tin từ server.

Đối soát tin gửi dùng server/client ID thay vì nội dung/thời gian, tránh gộp nhầm các tin giống nhau. Có thể bấm khối trích dẫn để cuộn tới tin gốc **đã nạp** trong hội thoại; tin chưa nạp hoặc ID không duy nhất sẽ được báo rõ. Xem [ADR-0006](docs/adr/0006-exact-message-identity-and-quote-navigation.md).

Ảnh nhận được hiển thị thumbnail và chú thích (nếu có), ẩn URL vận chuyển. Bấm ảnh để mở trình xem ảnh trong ứng dụng. Giờ tin mới, tin đồng bộ và mốc ngày dùng chung múi giờ Windows.

Đồng bộ điện thoại ưu tiên hội thoại hoạt động gần nhất: tải khoảng 7 ngày quanh lần hoạt động cuối, rồi mở rộng 90 ngày và lịch sử còn lại. **Đồng bộ từ điện thoại** tiếp tục theo checkpoint đã lưu, chỉ yêu cầu phần mới và phần chưa hoàn tất; **Chỉ cập nhật tin gần đây** bỏ qua hai lượt lịch sử cũ. **Quét lại toàn bộ lịch sử điện thoại** đặt lại tiến độ, giữ nguyên tin đã lưu. Mỗi lần vẫn cần điện thoại chấp thuận.

Bản cũ chưa ghi checkpoint nên không thể suy ra đợt tải đã hoàn tất từ số tin hiện có. Với dữ liệu từ bản cũ, dùng chế độ cập nhật gần đây nếu chưa muốn tải lại phần lịch sử cũ. Thứ tự gói do điện thoại quyết định; ưu tiên này áp dụng cho khoảng thời gian được yêu cầu, không bảo đảm từng tin đến đúng thứ tự mới → cũ. Xem [ADR-0007](docs/adr/0007-recent-first-resumable-phone-sync.md).

## Trạng thái kiểm chứng và giới hạn

Bản mã nguồn tại [`c45882a`](https://github.com/bqqhuy/ctdozlite/commit/c45882a) đã qua [CI native Windows ARM64](https://github.com/bqqhuy/ctdozlite/actions/runs/41400539416): **261 kiểm thử đạt**, kiểm tra PE/metadata, ảnh framebuffer DX12 và quy trình ẩn/mở/thoát từ tray. CI dùng GPU phần mềm Microsoft Basic Render Driver. Kết quả cho các sửa tiếp theo được cập nhật trong [PR #2](https://github.com/bqqhuy/ctdozlite/pull/2).

Những điểm cần biết trước khi dùng:

- Upload đọc theo khối **1 MiB**, tối đa **hai tệp đang truyền**, các tệp còn lại chờ; hộp chọn Windows hỗ trợ nhiều tệp. Không đặt giới hạn MB/tệp ở ứng dụng; máy chủ vẫn có thể áp dụng giới hạn riêng. Giới hạn giải mã preview ảnh 16 MiB vẫn giữ.
- Chuyển tiếp hiện hỗ trợ **tin chữ và sticker**. Chuyển tiếp ảnh/tệp/video bị khóa kèm hướng dẫn tải và gửi lại tệp gốc; ứng dụng không gửi nhãn chữ rồi báo đã chuyển tiếp media.
- Tin gửi lỗi giữ trong hội thoại cùng cảnh báo và nút **Gửi lại**; nút biến mất sau xác nhận thành công. Popup Windows hoạt động khi minimize hoặc ẩn tray, theo tùy chọn thông báo/âm thanh và tắt tiếng hội thoại. Xem [ADR-0009](docs/adr/0009-outgoing-retry-streaming-and-windows-popup.md).
- Nếu máy chủ chưa xác nhận gửi/thu hồi, ứng dụng báo chưa rõ kết quả và không tự gửi lại thành tin thường. Kiểm tra hội thoại trước khi thử lại. Trên Windows, dấu chưa đọc ở tray được cập nhật cả khi cửa sổ ẩn; âm báo dùng tùy chọn thông báo hiện có. Xem [ADR-0008](docs/adr/0008-fail-closed-mutations-and-background-unread.md).
- **Cuộc gọi còn cần kiểm chứng thực tế:** codec và gói giao thức đã có kiểm thử; gọi đi/đến, gọi nhóm, NAT, Bluetooth và camera trên MateBook chưa được xác nhận đầy đủ với client Zalo chính thức.
- Đã bổ sung **Settings → Đồng bộ → Đồng bộ từ điện thoại**: cần mở Zalo di động và xác nhận. Tiến độ và nút dừng hiện ở cửa sổ chính. Người dùng đã xác nhận luồng đồng bộ ban đầu hoạt động với điện thoại thật; cơ chế ưu tiên khoảng thời gian và tiếp tục mới vẫn cần kiểm chứng trên thiết bị; chỉ lấy được dữ liệu điện thoại còn giữ, không bảo đảm media cũ còn hạn. Xem [ADR-0005](docs/adr/0005-phone-approved-history-sync.md).
- Chưa có nguồn presence đáng tin cậy để hiển thị trạng thái online của mọi liên hệ.
- Một số Settings kế thừa chưa có hành vi đầy đủ; chưa bật updater tự động, Social Feed, scheduler hoặc REST/MCP của upstream trong bản tích hợp này.
- Database hiện dùng **schema 5**, bổ sung checkpoint đồng bộ điện thoại. Bản cũ hỗ trợ schema 2/3/4 sẽ từ chối mở; xem rollback trong [ADR-0007](docs/adr/0007-recent-first-resumable-phone-sync.md).

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
