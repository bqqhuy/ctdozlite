# Kế hoạch Triển khai Spec 011 (v0.010-arm)

## Kiến trúc Hệ thống

```
+-------------------------------------------------------------+
|                      egui UI (Main Thread)                  |
|  - Tab "Kho Lưu Trữ Server" (Ảnh/Video, Tệp, Link)         |
|  - Hộp thoại zCloud Viewer & Trạng thái tải Cloud           |
+------------------------------+------------------------------+
                               | WorkerCmd::*
                               v
+-------------------------------------------------------------+
|                   Tokio Runtime (Worker Thread)             |
|  - src/zcloud.rs (A20): getCloudViewerKey, verify, download |
|  - src/server_media.rs (B05): mediastore/list, oneone/list  |
|  - src/phone_sync/ (B11): transactional import & BinNet    |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|               Zalo HTTPS Services & Local SQLite Store      |
|  - zcld.chat.zalo.me / tt-files-wpa.zaloapp.com             |
|  - tt-chat3-wpa / tt-group-wpa                              |
|  - Local SQLite Store (short batch transactions)           |
+-------------------------------------------------------------+
```

## Các giai đoạn thực hiện:
1. **Giai đoạn 1**: Xây dựng module `src/zcloud.rs` (Data models, API, Unit tests).
2. **Giai đoạn 2**: Xây dựng module `src/server_media.rs` (Data models, API nhóm/1-1, Unit tests).
3. **Giai đoạn 3**: Giao dịch theo lô của Desktop V2 và import điện thoại theo ADR-0031; BinNet parser trong `src/phone_sync/`. Không sao chép đè DB đang mở để rollback.
4. **Giai đoạn 4**: Tích hợp UI vào `src/gui/tools_panel.rs` và `src/gui/worker.rs`.
5. **Giai đoạn 5**: Kiểm chứng tự động (Unit tests), review code và nghiệm thu trên Windows 11 ARM64 CI.
