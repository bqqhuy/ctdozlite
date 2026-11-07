# Tiêu chí Nghiệm thu Khối R15 (v0.011-arm)

## 1. Tiêu chí Kiểm thử Tự động (Unit Tests)
- [x] `test_parse_product_catalog`: Bóc tách danh mục sản phẩm từ JSON API Zalo.
- [x] `test_business_profile_verified_badge`: Kiểm tra hiển thị đúng nhãn tài khoản xác thực OA.
- [x] `test_parse_two_factor_status`: Bóc tách trạng thái 2FA từ response Zalo.
- [x] `test_lan_peer_discovery`: Bóc tách bản tin discovery UDP LAN peer.
- [x] `test_lan_transfer_state`: Kiểm tra chu trình truyền tệp LAN (Progress, Finished, Cancelled).
- [x] `test_service_resolver_fallback`: Kiểm tra fallback domain chính xác khi service chính không khả dụng.

## 2. Tiêu chí Kiểm thử trên Thiết bị Thật (Windows 11 ARM64 Native)
- [ ] Mở tab "Doanh nghiệp & Truyền LAN": kiểm tra tải danh mục sản phẩm của tài khoản.
- [ ] Kiểm tra trạng thái 2FA của tài khoản trong mục bảo mật.
- [ ] Thử tính năng quét thiết bị cùng mạng LAN (LAN Discovery): hiển thị IP/port của máy tính khác.
- [ ] Kiểm tra tính tương thích dịch vụ và không phát sinh crash Direct3D 12.
