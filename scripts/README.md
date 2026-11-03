# Công cụ Windows ARM64

`build-windows-arm64.ps1` chạy test native, build PE ARM64, kiểm tra DLL, thử GUI và tạo bộ cài/portable/checksum. Chạy trong PowerShell trên Windows 11 ARM64; xem [README](../README.md).

`decode_voip_log.py` hỗ trợ phân tích log giao thức đã thu thập có chủ đích. Log có thể chứa dữ liệu riêng tư; không đưa lên issue công khai.

`xwin-build-arm64.sh` build chéo bản ARM64 từ Linux bằng `cargo-xwin` để kiểm tra nhanh (`./scripts/xwin-build-arm64.sh build --bin ctdozlite` hoặc `... clippy --all-targets -- -D warnings`). Script tải CRT MSVC và Windows SDK của Microsoft về `~/.cache/cargo-xwin`; chạy script là chấp nhận giấy phép của chúng. Bản phát hành vẫn build và test native bằng `build-windows-arm64.ps1`.
