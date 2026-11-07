# Kế hoạch Triển khai Spec 012 (v0.011-arm)

## Kiến trúc Hệ thống

```
+-------------------------------------------------------------+
|                      egui UI (Main Thread)                  |
|  - Tab "Doanh nghiệp & Truyền LAN" trong Tools Panel        |
|  - Danh mục sản phẩm (Product Catalog) & Badge OA           |
|  - Danh sách thiết bị cùng mạng LAN (LAN Discovery)        |
+------------------------------+------------------------------+
                               | WorkerCmd::*
                               v
+-------------------------------------------------------------+
|                   Tokio Runtime (Worker Thread)             |
|  - src/business.rs (C01): /api/prodcatalog/*                |
|  - src/account_recovery.rs (C02): 2FA & change_pwd          |
|  - src/lan_transfer.rs (C03): LAN discovery & transfer     |
|  - src/service_resolver.rs (C04): Unified endpoint resolver |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                  Zalo Services & LAN Sockets                |
|  - tt-profile-wpa / tt-files-wpa / LAN UDP Discovery        |
+-------------------------------------------------------------+
```

## Các giai đoạn triển khai:
1. **Giai đoạn 1**: Xây dựng module `src/business.rs` (C01).
2. **Giai đoạn 2**: Xây dựng module `src/account_recovery.rs` (C02).
3. **Giai đoạn 3**: Xây dựng module `src/lan_transfer.rs` (C03).
4. **Giai đoạn 4**: Xây dựng module `src/service_resolver.rs` (C04).
5. **Giai đoạn 5**: Tích hợp UI vào `src/gui/tools_panel.rs` và `src/gui/worker.rs`.
6. **Giai đoạn 6**: Đồng bộ 4 điểm metadata release `v0.011-arm` và chạy CI Windows 11 ARM64.
