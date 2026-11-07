# Đặc tả Kỹ thuật Spec 012: Business, OA, Bảo mật Tài khoản, LAN Transfer và Tương thích Dịch vụ (v0.011-arm)

Ngày: 07/11/2026 (Asia/Saigon)
Khối triển khai: **R15**
Mã phạm vi Parity: **C01, C02 (phần còn lại), C03, C04 (xuyên suốt)**

## 1. Mục tiêu và Đối chiếu Zalo gốc (PC & Web)

### 1.1. C01 — Business / OA / Enterprise / Product Catalog
- **Zalo gốc**:
  - Hỗ trợ tài khoản Doanh nghiệp (Business Account) và Zalo Official Account (OA).
  - Quản lý danh mục sản phẩm / gian hàng (Product Catalog) qua `/api/prodcatalog/*` (`/api/prodcatalog/list`, `/api/prodcatalog/item/create`, `/api/prodcatalog/item/update`, `/api/prodcatalog/item/delete`).
  - Hiển thị nhãn xác thực OA (Verified badge) và kiểm tra entitlement của tài khoản.
- **ctdozlite**: Cung cấp module `src/business.rs` chứa data structures `ProductCatalogItem`, `BusinessProfile`, `AccountCapability` và API gọi dịch vụ quản lý danh mục sản phẩm Zalo.

### 1.2. C02 — Bảo mật & Khôi phục tài khoản (Account Security & 2FA)
- **Zalo gốc**:
  - Kiểm tra trạng thái kích hoạt xác thực 2 bước (2FA / Two-Factor Authentication) qua `/api/login/check2fa`.
  - Hỗ trợ cập nhật mật khẩu đăng nhập `/api/login/change_pwd` với mã hoá mật khẩu cũ/mới an toàn.
  - Phân định rõ ràng các chức năng thực hiện được trên client Desktop và các chức năng cần chuyển hướng app điện thoại (eKYC).
- **ctdozlite**: Cung cấp module `src/account_recovery.rs` với các helper kiểm tra 2FA và bảo mật tài khoản.

### 1.3. C03 — Truyền tệp Mạng Cục bộ (LAN / Local Transfer Protocol)
- **Zalo gốc**:
  - Zalo PC hỗ trợ tính năng truyền tệp siêu tốc giữa các máy tính cùng kết nối chung một mạng LAN/Wi-Fi qua giao thức discovery tín hiệu (`/api/lan/discover`, `/api/lan/transfer`).
  - Tự động nhận diện thiết bị ngang hàng (peer) trong mạng nội bộ để chuyển trực tiếp dữ liệu tệp tin dung lượng lớn mà không làm nghẽn băng thông Internet hay phụ thuộc hạn mức máy chủ Zalo.
- **ctdozlite**: Cung cấp module `src/lan_transfer.rs` với struct `LanPeer`, `LanTransferSession`, và giao thức báo hiệu (signaling) phát hiện peer.

### 1.4. C04 — Tương thích Dịch vụ & Điều phối Endpoint (Service Resolver & Fallback)
- **Zalo gốc**: Các endpoint API có thể phân nhánh giữa v2 và service map fallback (`zpw_service_map_v3`).
- **ctdozlite**: Cung cấp module `src/service_resolver.rs` đóng vai trò bộ phân giải URL tập trung, tự động tìm kiếm domain chính xác từ service map và fallback an toàn khi gặp sự cố máy chủ.
