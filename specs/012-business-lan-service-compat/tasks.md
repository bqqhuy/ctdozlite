# Danh sách Nhiệm vụ Khối R15 (Spec 012 / v0.011-arm)

## 1. Khối C01: Business / OA / Enterprise / Product Catalog
- [x] [R15-T01] [C01] Mô hình dữ liệu `ProductCatalogItem`, `BusinessProfile`, `AccountCapability`.
- [x] [R15-T02] [C01] API lấy danh mục sản phẩm `/api/prodcatalog/list` và cập nhật sản phẩm.
- [x] [R15-T03] [C01] Bộ phân giải JSON phản hồi và unit tests kiểm tra parsing sản phẩm & hồ sơ OA.

## 2. Khối C02: Bảo mật & Khôi phục tài khoản
- [x] [R15-T04] [C02] Mô hình dữ liệu `SecuritySettings`, `TwoFactorStatus`.
- [x] [R15-T05] [C02] API kiểm tra 2FA `/api/login/check2fa` và đổi mật khẩu an toàn `/api/login/change_pwd`.
- [x] [R15-T06] [C02] Unit tests cho kiểm tra bảo mật tài khoản.

## 3. Khối C03: Truyền tệp LAN (Local Transfer Protocol)
- [x] [R15-T07] [C03] Mô hình dữ liệu `LanPeer`, `LanTransferState`, `LanTransferSession`.
- [x] [R15-T08] [C03] Giao thức discovery mạng LAN và báo hiệu truyền tệp nội bộ.
- [x] [R15-T09] [C03] Unit tests kiểm tra phát hiện peer và quản lý trạng thái phiên truyền tệp LAN.

## 4. Khối C04: Tương thích dịch vụ & Điều phối endpoint
- [x] [R15-T10] [C04] Bộ phân giải URL tập trung `ServiceResolver` với fallback an toàn.
- [x] [R15-T11] [C04] Unit tests cho `ServiceResolver`.

## 5. Tích hợp UI & Worker Command
- [x] [R15-T12] Thêm các lệnh `WorkerCmd::GetBusinessCatalog` và `WorkerCmd::DiscoverLanPeers` vào `src/gui/worker.rs`.
- [x] [R15-T13] Thêm tab "Doanh nghiệp & Truyền LAN" vào `src/gui/tools_panel.rs`.
- [x] [R15-T14] Đăng ký modules trong `src/lib.rs` và kiểm chứng toàn diện.
