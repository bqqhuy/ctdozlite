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
|  - src/phone_sync/ (B11): atomic backup restore & BinNet    |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|               Zalo HTTPS Services & Local SQLite Store      |
|  - zcld.chat.zalo.me / tt-files-wpa.zaloapp.com             |
|  - tt-chat3-wpa / tt-group-wpa                              |
|  - Local SQLite Store (Snapshot & Rollback Transaction)     |
+-------------------------------------------------------------+
```

## Các giai đoạn thực hiện:
1. **Giai đoạn 1**: Xây dựng module `src/zcloud.rs` (Data models, API, Unit tests).
2. **Giai đoạn 2**: Xây dựng module `src/server_media.rs` (Data models, API nhóm/1-1, Unit tests).
3. **Giai đoạn 3**: Cải tiến atomic snapshot & rollback và BinNet parser trong `src/phone_sync/`.
4. **Giai đoạn 4**: Tích hợp UI vào `src/gui/tools_panel.rs` và `src/gui/worker.rs`.
5. **Giai đoạn 5**: Kiểm chứng tự động (Unit tests), review code và nghiệm thu trên Windows 11 ARM64 CI.
