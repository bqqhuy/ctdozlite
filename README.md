# ctdozlite — Windows 11 ARM64

Fork dành riêng cho **Windows 11 ARM64** (Snapdragon / Windows on Arm), viết bằng Rust. Gói phát hành là PE ARM64 native, không chạy qua giả lập x64. Dựa trên [ctdoteam1110/ctdozlite](https://github.com/ctdoteam1110/ctdozlite); đây là client không chính thức, không được Zalo bảo trợ.

[Build Windows ARM64](https://github.com/bqqhuy/ctdozlite/actions/workflows/windows-arm64.yml) · [Releases](https://github.com/bqqhuy/ctdozlite/releases) · [Hướng dẫn](docs/WINDOWS_ARM64.md) · [Audit và giới hạn](docs/ARM64_IMPLEMENTATION.md)

## Cài đặt

Yêu cầu Windows 11 ARM64, build 22000 trở lên. Chọn `*-setup-windows-arm64.exe` để cài hoặc `*-portable-windows-arm64.exe` để chạy trực tiếp. Kiểm tra SHA-256 trong `SHA256SUMS-windows-arm64.txt` đi kèm đúng bản phát hành. Artifact trên Actions là bản kiểm thử của commit/PR, chưa mặc nhiên là bản ổn định.

Khởi chạy GUI mặc định và đăng nhập bằng QR. Không cần Node.js, Electron, FFmpeg hay VC++ Redistributable riêng. Video sử dụng Media Foundation/H.264 của Windows; Windows N cần Media Feature Pack. Cho phép ứng dụng desktop truy cập microphone/camera trong Windows Settings → Privacy & security.

## Chức năng và phạm vi

- Giao diện Rust/egui, renderer Direct3D 12; khung cửa sổ Windows hỗ trợ kéo, resize, Snap và Snap. X/Alt+F4 ẩn xuống tray; click trái hoặc phải icon để chọn **Mở Apps** / **Thoát Apps**.
- Nhắn tin, nhóm, tệp/ảnh/sticker và lịch sử SQLite theo tài khoản; phiên đăng nhập được bảo vệ bằng Windows DPAPI.
- Cập nhật hội thoại/tin gần đây tách riêng với tải lịch sử nhóm và Cloud theo trang. Có tiến độ, dừng và tiếp tục; không cam kết lấy lại dữ liệu server đã xóa hoặc toàn bộ lịch sử chat cá nhân từ điện thoại.
- Audio WASAPI qua CPAL/Opus; camera và H.264 qua Media Foundation trong tiến trình ARM64. Cuộc gọi dùng giao thức riêng của Zalo; khả năng tương tác cuộc gọi đi/đến cần xác nhận với tài khoản và thiết bị thật. Gọi nhóm chưa được hỗ trợ.
- Không hiển thị người khác online khi chưa có dữ liệu presence thực. Một số tùy chọn kế thừa chỉ lưu trên máy, chưa thay đổi thiết lập quyền riêng tư phía server; xem audit.

Build/test tự động không thay thế kiểm tra đăng nhập, đồng bộ và cuộc gọi trên máy người dùng. Xem [ma trận kiểm chứng](docs/ARM64_IMPLEMENTATION.md) trước khi dùng làm client chính.

## Build native

Trên Windows 11 ARM64: cài Visual Studio 2022 Build Tools với ARM64 MSVC + Windows SDK, Rust ARM64, CMake, LLVM và Inno Setup 6.7.1 trở lên. Toolchain Rust được ghim trong `rust-toolchain.toml`.

```powershell
rustup set default-host aarch64-pc-windows-msvc
./scripts/build-windows-arm64.ps1
```

Script chạy test, build release, kiểm tra PE `0xAA64`, dependency DLL, khởi chạy GUI kiểm thử và đóng gói vào `dist/`. Workflow duy nhất tạo bộ cài và portable Windows ARM64. Tài liệu giao thức và kiểm thử debug được giữ để bảo trì; chúng không phải nền tảng phát hành khác.

## Mã nguồn và giấy phép

MIT hoặc Apache-2.0; giữ ghi nhận tác giả upstream trong Git history và các file giấy phép. [Giao thức](docs/GIAO_THUC_ZALO.md), `re-notes/` và công cụ đọc log phục vụ chẩn đoán; không đưa log chứa dữ liệu tài khoản lên issue công khai.

Zalo và các nhãn hiệu liên quan thuộc VNG. Fork cộng đồng này độc lập, không phải ứng dụng chính thức và không được VNG bảo trợ. Ghi nhận tác giả dự án gốc: [ctdoteam1110/ctdozlite](https://github.com/ctdoteam1110/ctdozlite).

Ghi nhận đóng góp upstream của Thuong Tin và các tác giả khác; lịch sử Git được giữ nguyên.
