# Danh sách Nhiệm vụ Khối R14 (Spec 011 / v0.010-arm)

## 1. Khối A20: zCloud media & Khôi phục dữ liệu Cloud
- [x] [R14-T01] [A20] Mô hình dữ liệu zCloud (`CloudViewerKey`, `CloudMediaItem`, `CloudFileStatus`).
- [x] [R14-T02] [A20] API lấy viewer key `/api/getCloudViewerKey` và verify `/cloudmedia/queue/pc/verify`.
- [x] [R14-T03] [A20] API lấy signed download URLs `/cloudmedia/downloadurls/pc/v1` và unit tests.

## 2. Khối B05: Kho Media Server (Server Media Library)
- [x] [R14-T04] [B05] Mô hình dữ liệu `ServerMediaItem`, `ServerMediaType` (Photo, Video, File, Link) và `ServerMediaPage`.
- [x] [R14-T05] [B05] API lấy kho lưu trữ nhóm `/api/mediastore/list` và 1-1 `/api/media/oneone/list`.
- [x] [R14-T06] [B05] Bộ phân giải JSON phản hồi và unit tests kiểm tra phân trang và lọc theo loại media.

## 3. Khối B11: V1/BinNet và Khôi phục dữ liệu an toàn
- [x] [R14-T07] [B11] Cơ chế Atomic Snapshot & Rollback cho Desktop V2 import trong `src/phone_sync/import.rs`.
- [x] [R14-T08] [B11] Parser BinNet frame descriptor bảo toàn nguyên vẹn `raw/Unknown` bytes trong `src/phone_sync/codec.rs`.

## 4. Tích hợp UI & Worker Command
- [x] [R14-T09] Tích hợp lệnh `WorkerCmd::FetchServerMedia` và `WorkerCmd::GetCloudViewerKey` trong `src/gui/worker.rs`.
- [x] [R14-T10] Giao diện xem Kho Lưu Trữ Server (Ảnh/Video, Tệp, Link) trong `src/gui/tools_panel.rs`.
- [x] [R14-T11] Đăng ký modules trong `src/lib.rs` và kiểm chứng toàn diện.
