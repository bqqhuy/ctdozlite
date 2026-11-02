# Windows 11 ARM64

Bản fork này chỉ phát hành Windows 11 ARM64, build 22000 trở lên. Executable Rust/MSVC và codec trong tiến trình đều chạy native ARM64. Bộ cài từ chối kiến trúc khác. Bản chưa ký Authenticode; Windows có thể hiện cảnh báo SmartScreen.

## Cài đặt và dữ liệu

Chọn portable hoặc setup từ [Releases của fork](https://github.com/bqqhuy/ctdozlite/releases). Đối chiếu file với checksum của đúng bản đó. Artifact trong Actions là bản kiểm thử theo commit; đọc kết quả workflow trước khi dùng.

- Dữ liệu mặc định: `%LOCALAPPDATA%\ctdozlite`, không phụ thuộc thư mục cài hoặc thư mục làm việc.
- `session.dat` dùng DPAPI của người dùng Windows hiện tại. Không chép sang tài khoản/máy khác.
- `accounts/<sha256-id>/` chứa lịch sử SQLite và cache riêng cho tài khoản. SQLite chưa mã hóa ở mức ứng dụng.
- Tắt **Ghi nhớ đăng nhập** xóa phiên đã lưu và chặn ghi phiên mới. Đăng xuất giữ lịch sử trong vùng của tài khoản cũ, không thu hồi phiên trên máy chủ.
- Mỗi thư mục dữ liệu chỉ cho chạy một instance. `--portable` dùng thư mục `data` cạnh exe; cần quyền ghi. `CTDOZ_DATA_DIR` chỉ định thư mục tuyệt đối khác.
- Session JSON và lịch sử cũ không có thông tin chủ sở hữu không được tự nhập. Giữ chúng riêng và đăng nhập lại bằng QR.

Nút X và Alt+F4 ẩn cửa sổ xuống tray. Click trái hoặc phải icon để mở menu **Mở Apps** / **Thoát Apps**. Chọn Thoát Apps hoặc Ctrl+Q để tắt hẳn. Nếu không tạo được tray, nút X sẽ thoát để tránh tiến trình không thể mở lại. Khung cửa sổ Windows hỗ trợ kéo, resize và Snap.

## Hội thoại và lịch sử

**Cập nhật hội thoại** lấy danh bạ, nhóm và tin gần đây; không đồng nghĩa sao chép toàn bộ tin từ điện thoại.

Sau đăng nhập, ứng dụng tự tải hội thoại, tên nhóm, tin gần đây và lịch sử nhóm/Cloud mà máy chủ cho phép. Trong Settings → Đồng bộ, chọn **Tải / tiếp tục lịch sử** để tải nhóm và Cloud theo trang vào SQLite. Có tiến độ và nút dừng. Mỗi lượt tối đa 200 trang/hội thoại, 50 tin/trang, cách nhau ít nhất 300 ms. Bấm tiếp tục để tải thêm. Vị trí được lưu sau khi tin nhắn đã ghi vào database, nên đóng ứng dụng giữa chừng không làm mất dữ liệu đã ghi.

Tải nhóm/Cloud chỉ lấy được tin máy chủ còn cung cấp. Muốn yêu cầu lịch sử từ điện thoại, chọn **Đồng bộ từ điện thoại**, mở Zalo di động và xác nhận **Đồng bộ ngay**. Tiến độ và nút dừng hiện tại cửa sổ chính. Luồng này chưa có kiểm chứng đầu-cuối với điện thoại thật; dữ liệu đã lưu được giữ khi dừng hoặc mất mạng. Chỉ khôi phục tin điện thoại còn giữ; media phụ thuộc liên kết còn hạn. Lần đầu cần tải bộ giải mã chính thức có SHA-256 cố định; module không được đóng gói trong installer. Nhóm chưa nhận diện đúng ID được giữ riêng, không gán nhầm thành hội thoại gửi tin. Xem ADR-0005.

Tên/ảnh đại diện không được dùng để gộp hai ID khác nhau. Bản cũ đã trộn lịch sử thì không có đủ bằng chứng để tự tách lại an toàn; bản mới ngăn phát sinh thêm. Không xóa database để chữa lỗi này.

Không có nguồn presence đã xác minh, nên ứng dụng không gắn chấm xanh hay chữ “vừa truy cập” cố định cho mọi liên hệ. Cài đặt quyền riêng tư trên server phải thay đổi bằng ứng dụng Zalo chính thức.

## Microphone, camera và video

Audio dùng WASAPI qua CPAL, mã hóa Opus native, hỗ trợ các kiểu mẫu số nguyên và số thực của CPAL. Camera, H.264 encode/decode và preview dùng **Windows Media Foundation**, không gọi FFmpeg hoặc ZaloCall.exe.

Trong Windows Settings → Privacy & security → Camera/Microphone, bật quyền truy cập và **Let desktop apps access…**. Chọn thiết bị trong Settings của ứng dụng. Đóng ứng dụng khác đang giữ camera. Nếu lỗi, cấp quyền/chọn thiết bị rồi bật camera lại.

Windows N cần cài Media Feature Pack. Thiếu thành phần media sẽ báo lỗi chức năng camera/video; các DLL media được delay-load để không chặn khởi động GUI. Camera ưu tiên RGB thay cho IR/Hello, thương lượng NV12 ở độ phân giải driver hỗ trợ rồi chuẩn hóa stride, scale/letterbox về 320×240. Nhịp gửi 15 fps được giới hạn sau capture, không ép camera phải có mode 15 fps. Camera không trả frame trong 5 giây sẽ báo lỗi; chưa xác nhận driver máy đích. Audio giữ pha resampling giữa callback và giải mã theo số mẫu Opus thực tế; lỗi mic/loa được báo riêng để video vẫn có thể hoạt động.

Cuộc gọi nhóm chưa hỗ trợ. Gọi 1-1 không bị khóa tài khoản thử nghiệm mặc định. Chỉ bật `ZALO_CALL_TEST_LOCK=1` khi chủ động dùng allowlist chẩn đoán. Test codec tự động không chứng minh cuộc gọi tới ứng dụng Zalo chính thức hoạt động trên mọi mạng/thiết bị.

## Chẩn đoán

```powershell
.\ctdozlite.exe --doctor
.\ctdozlite.exe --licenses
$env:CTDOZ_RENDERER = 'dx12'
.\ctdozlite.exe --gui --dev
```

DX12 là renderer mặc định. `CTDOZ_RENDERER=glow` chọn OpenGL nếu máy có driver phù hợp; một số máy cần OpenGL Compatibility Pack. Cập nhật Windows và driver GPU từ nhà sản xuất khi gặp màn hình đen.

Log mặc định tắt. `--dev` hoặc `ZALO_DIAGNOSTICS=1` bật chẩn đoán trong thư mục `diagnostics` của data directory; log cuộc gọi xoay vòng ở 2 MiB. Log có thể chứa dữ liệu riêng tư, không đăng công khai nguyên trạng. `ZALO_VOIP_LOG=off` tắt log gói tin.

Settings co theo diện tích và DPI, dùng danh sách chọn trang khi cửa sổ hẹp; nội dung có thanh cuộn. Font đọc từ Windows, gồm fallback CJK khi đã cài. Nếu chữ Trung/Nhật/Hàn vẫn thành ô vuông, cài font/language features tương ứng của Windows.

## Build và kiểm thử

Cần Rust ARM64, Visual Studio 2022 Build Tools (MSVC ARM64 + Windows SDK), CMake, LLVM và Inno Setup 6.7.1 trở lên. Toolchain Rust được ghim trong `rust-toolchain.toml`.

```powershell
rustup set default-host aarch64-pc-windows-msvc
.\scripts\build-windows-arm64.ps1
```

Workflow duy nhất chạy trên `windows-11-arm`: test native, build release, kiểm tra PE `0xAA64`, kiểm tra DLL phụ thuộc, khởi chạy `--version`, thử GUI DX12 ở ba kích thước/DPI, kiểm tra vòng đời tray trên cửa sổ thật và đóng gói. Ảnh PNG và log giúp kiểm tra bố cục; file manifest ghi commit/toolchain. Chỉ tag/release hợp lệ mới đính kèm binary vào Release.

Các test GUI dùng dữ liệu giả trong data directory riêng, không khôi phục phiên, không tạo QR, không gọi mạng hoặc gửi tin/gọi người thật. Vẫn cần kiểm tra trên thiết bị ARM64 thật: GPU, camera, mic/Bluetooth, resize/Snap, nhiều màn hình, sleep/resume, cài/gỡ và cuộc gọi với tài khoản được phép.

Máy đích do người dùng cung cấp: **Huawei MateBook E Go Gen 3 — Snapdragon 8cx Gen 3**. Chưa có kết quả thử GPU/camera trên thiết bị này; CI dùng GPU phần mềm Microsoft Basic Render Driver.
