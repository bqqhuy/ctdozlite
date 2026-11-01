# Windows 11 ARM64

ctdozlite dùng Rust MSVC ARM64 native, renderer Direct3D 12 và dialog Win32. OpenGL là renderer dự phòng. Bản phát hành phải vượt qua CI Windows ARM64; build thành công chưa thay thế được kiểm thử GPU, microphone, camera và tài khoản thật trên thiết bị.

## Build và tải artifact

Trong Actions của fork, chọn **Windows ARM64 (native build)** → **Run workflow**. Workflow cũng chạy khi có PR/main và tag v*. Artifact `ctdozlite-windows-arm64` gồm portable, bộ cài, checksum SHA256, thông tin commit/toolchain và license dự án. Chỉ workflow tag/release đính kèm assets vào GitHub Release.

Build trực tiếp trên Windows ARM64 cần Visual Studio Build Tools 2022 với Desktop development with C++, MSVC ARM64, Windows SDK, Clang và CMake; cài Rust ARM64 native và Inno Setup 6.7.1 trở lên. Toolchain Rust được ghim trong `rust-toolchain.toml`.

```powershell
rustup set default-host aarch64-pc-windows-msvc
rustup toolchain install 1.99.1-aarch64-pc-windows-msvc --profile minimal
.\scripts\build-windows-arm64.ps1
```

Script chạy test tất cả target, build `aarch64-pc-windows-msvc`, kiểm tra PE `0xAA64`, thực thi `--version`, rồi tạo bộ cài và checksum. Bản build chưa ký Authenticode; chữ ký cần chứng chỉ riêng của nhà phát hành.

## Dữ liệu và phiên đăng nhập

- Mặc định: `%LOCALAPPDATA%\ctdozlite`, độc lập thư mục cài và thư mục làm việc. Bộ cài per-user và per-machine dùng cùng cách lưu theo người dùng.
- `session.dat`: Windows DPAPI theo người dùng hiện tại, không dùng bảo vệ theo máy. Không chia sẻ file này. DPAPI không bảo vệ khỏi phần mềm độc hại đang chạy dưới chính tài khoản Windows đó.
- `accounts/<sha256-id>/`: lịch sử SQLite và cache hội thoại riêng theo tài khoản Zalo. Logout thu hồi phiên cục bộ và giữ lịch sử trong namespace cũ; tài khoản khác không nạp namespace đó. Logout cục bộ không phải thu hồi phiên trên máy chủ Zalo.
- Tắt **Ghi nhớ tài khoản đăng nhập lần trước** sẽ xóa phiên đã lưu và ngăn mọi đường đăng nhập/làm mới ghi lại. Đóng rồi mở app phải đăng nhập lại.
- Chỉ cho phép một instance trên mỗi data directory để tránh ghi chồng.
- `--portable` lưu vào thư mục `data` cạnh executable. Không dùng portable trong Program Files hoặc thư mục không có quyền ghi.
- `CTDOZ_DATA_DIR` cho phép chỉ định data directory tuyệt đối.

Bản cũ lưu JSON, SQLite và cache theo thư mục làm việc. Bản mới **không tự nhập session/lịch sử cũ không có thông tin chủ tài khoản**. Hãy đăng nhập lại bằng QR; dữ liệu cũ vẫn được giữ tại chỗ. Không chép database cũ vào namespace tài khoản khi chưa xác minh đúng chủ sở hữu.

Trên Linux/macOS, session được lưu ở thư mục người dùng bằng file quyền 0600; DPAPI chỉ áp dụng Windows. Lịch sử SQLite chưa mã hóa ở mức ứng dụng.

## Video hoàn toàn native ARM64

Audio dùng `cpal` và Opus native. Video cần FFmpeg ngoài app. FFmpeg x64 bị từ chối trên Windows ARM64 để tránh giả lập.

Chọn bản FFmpeg **ARM64 Windows** có DirectShow (`dshow`), encoder `libx264`, decoder H.264 và filter scale/pad/fps/format. Xác minh nguồn tải, hash, các DLL đi kèm và điều kiện phân phối của đúng binary. Repo không tự tải hoặc đóng gói FFmpeg chưa xác minh.

Có thể đặt bản đó tại `ffmpeg-arm64\ffmpeg.exe` cạnh app, hoặc chỉ định đường dẫn tuyệt đối:

```powershell
$env:CTDOZ_FFMPEG = 'C:\Tools\ffmpeg-arm64\bin\ffmpeg.exe'
.\zalo-rust.exe --doctor
```

Chương trình tìm tiếp trong PATH tuyệt đối nếu không có cấu hình riêng. `--doctor` báo đường dẫn dữ liệu và trạng thái FFmpeg; kiểm tra header PE và khả năng codec không thay thế kiểm thử capture trên camera thật. Trước khi gọi video, app kiểm tra các khả năng bắt buộc và hiển thị lỗi nếu không đủ. Chat và gọi thoại không cần FFmpeg.

Cuộc gọi 1-1 không còn bị khóa mặc định vào các tài khoản thử nghiệm. Gọi nhóm chưa hỗ trợ. Chỉ bật `ZALO_CALL_TEST_LOCK=1` nếu muốn dùng lại chính sách allowlist thử nghiệm; `ZALO_CALL_ALLOW_IDS` bổ sung ID cho chính sách đó.

## Chẩn đoán

```powershell
$env:CTDOZ_RENDERER = 'wgpu' # DX12 trên Windows ARM64
.\zalo-rust.exe --dev
$env:CTDOZ_RENDERER = 'glow' # OpenGL nếu driver hỗ trợ
.\zalo-rust.exe --dev
```

Log signaling và gói tin cuộc gọi mặc định tắt. `--dev` hoặc `ZALO_DIAGNOSTICS=1` bật chẩn đoán; log nằm trong `diagnostics` dưới data directory và xoay vòng ở 2 MiB. Dữ liệu chẩn đoán có thể chứa thông tin cuộc gọi; không chia sẻ nguyên trạng. `ZALO_VOIP_LOG=off` tắt log gói tin.

Audio hỗ trợ F32/I16/U16/I32/F64 theo format thiết bị. Lỗi quyền microphone, thiếu thiết bị hoặc không mở được stream được đưa lên trạng thái GUI. Vẫn cần kiểm thử Bluetooth/USB, đổi thiết bị, sleep/resume, Remote Desktop và DPI nhiều màn hình trên phần cứng ARM64.
