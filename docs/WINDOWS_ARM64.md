# ctdozlite trên Windows ARM64 (Snapdragon / Surface Pro X / Copilot+ PC)

Bản này build **native ARM64** (`aarch64-pc-windows-msvc`), không chạy qua lớp giả lập x64 (Prism).

## Thay đổi so với bản gốc

| Hạng mục | Bản gốc | Bản ARM64 |
|---|---|---|
| Đồ họa | Chỉ OpenGL (`glow`) | **Direct3D 12 qua wgpu** trước, OpenGL làm dự phòng |
| Hộp thoại chọn file | Gọi PowerShell + WinForms (chậm 1–3 s) | Hộp thoại Win32 gốc (`rfd`), mở tức thì |
| Chọn thư mục tải về | Phải chọn một file rồi lấy thư mục cha | Hộp thoại chọn thư mục thật |
| Icon trong .exe | Bị bỏ qua khi cross-compile (build.rs kiểm tra OS của máy build) | Kiểm tra OS của **target** |
| Profile release | Mặc định | `lto = "fat"`, `codegen-units = 1`, `strip = true` |
| Bộ cài | Chỉ x64 | `iscc /DArch=arm64` ra bộ cài ARM64; shortcut đặt `WorkingDir` |
| CI | Không có ARM64 | `.github/workflows/windows-arm64.yml` chạy trên runner `windows-11-arm` |

Các thay đổi chỉ kích hoạt trên Windows (riêng wgpu chỉ trên Windows ARM64); bản Linux/macOS/x64 giữ nguyên hành vi.

**Vì sao đổi sang Direct3D 12?** Máy Windows ARM không đảm bảo có driver OpenGL. Snapdragon X đời mới có OpenGL 4.6, nhưng các đời 8cx/SQ1/SQ2 phải cài thêm "OpenGL Compatibility Pack" (giả lập GL trên DX12), nếu không bản gốc có thể không mở được cửa sổ. DX12 luôn có sẵn.

## Cách 1: Build bằng GitHub Actions (không cần cài gì)

1. Fork repo lên tài khoản GitHub của bạn (repo public thì runner ARM được miễn phí).
2. Đẩy các thay đổi này lên nhánh `main` của bản fork.
3. Vào tab **Actions**, chọn **Windows ARM64 (native build)**, rồi bấm **Run workflow**.
4. Khi xong (khoảng 10–15 phút), tải artifact `ctdozlite-windows-arm64`. Bên trong có bản portable và bộ cài.

## Cách 2: Build trực tiếp trên máy Windows ARM

```powershell
# 1. Visual Studio Build Tools 2022: chọn "Desktop development with C++"
#    và thêm "MSVC ARM64 build tools", "C++ Clang tools for Windows", "CMake tools"
winget install Microsoft.VisualStudio.2022.BuildTools
# 2. Rust (bản ARM64 native)
winget install Rustlang.Rustup
rustup default stable-aarch64-pc-windows-msvc
# 3. Build (mở "Developer PowerShell for VS 2022" cho ARM64)
$env:CMAKE_POLICY_VERSION_MINIMUM = "3.5"
cargo build --release --bin zalo-rust
# Kết quả: target\release\zalo-rust.exe
```

`ring` (thư viện mã hóa TLS) cần `clang` khi build cho ARM64 Windows. Nếu báo lỗi thiếu clang, cài `winget install LLVM.LLVM`.

## Khắc phục sự cố

- **Cửa sổ không hiện / lỗi đồ họa:** chạy `set CTDOZ_RENDERER=glow` rồi mở lại để ép dùng OpenGL, hoặc `set CTDOZ_RENDERER=wgpu` để ép DX12. Thêm `--dev` để xem renderer đang dùng.
- **Gọi video:** vẫn cần `ffmpeg` trong PATH như bản gốc. Bản ffmpeg x64 vẫn chạy được qua giả lập. Bản ARM64 native nhẹ hơn nhưng phải tự tìm nguồn.
- **Dữ liệu phiên** (`session.json`, `zalo_history.db`) được lưu ở thư mục làm việc. Nếu cài cho mọi người dùng vào `Program Files`, thư mục đó không ghi được; hãy giữ mặc định cài cho riêng người dùng.
