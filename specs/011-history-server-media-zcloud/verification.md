# Tiêu chí Nghiệm thu Khối R14 (v0.010-arm)

## 1. Tiêu chí Kiểm thử Tự động (Unit Tests)
- [x] `test_parse_cloud_viewer_key`: Bóc tách thành công key và expiration từ phản hồi `/api/getCloudViewerKey`.
- [x] `test_cloud_file_status`: Kiểm tra đúng trạng thái `Available`, `Expired`, `NotInCloud`, `Downloading`.
- [x] `test_parse_cloud_download_urls`: Bóc tách danh sách download URLs từ zCloud.
- [x] `test_parse_server_media_list_photos`: Bóc tách danh sách ảnh/video từ kho lưu trữ server.
- [x] `test_parse_server_media_list_files`: Bóc tách danh sách tệp đính kèm kèm kích thước và tên file.
- [x] `test_parse_server_media_list_links`: Bóc tách danh sách liên kết web từ kho lưu trữ.
- [x] Desktop V2 regression qua `dry_run_import_replay_delete_and_account_binding`: giữ live WAL/committed batch/media khi lô sau lỗi, xóa current batch/media, pre-cancel và retry idempotent. Thay test byte-file `test_atomic_backup_rollback` đã gỡ; `cargo test --locked --test desktop_backup_v2 -j 1 -- --test-threads=1` đạt, exit 0 ngày 19/11/2026. Không thay thế nghiệm thu thiết bị.
- [x] `test_binnet_unknown_preservation`: Kiểm tra bảo toàn dữ liệu `Unknown` trong frame BinNet.

## 2. Tiêu chí Kiểm thử trên Thiết bị Thật (Windows 11 ARM64 Native)
- [ ] Mở tab "Kho Media Server & zCloud": kiểm tra tải đúng danh sách ảnh, tệp, link của hội thoại đã chọn.
- [ ] Tải một tệp từ kho lưu trữ: kiểm tra tiến trình tải về và mở file bình thường.
- [ ] Thử mở một tệp zCloud: hiển thị thông báo trạng thái hoặc mở file xem trước đúng chuẩn Zalo.
- [ ] Thử import backup với file không hợp lệ: kiểm tra CSDL SQLite gốc không bị ảnh hưởng, tự động hoàn tác.
