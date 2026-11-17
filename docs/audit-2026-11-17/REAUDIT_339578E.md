# Tái kiểm `339578e` — kết quả sửa R01–R08

Ngày 17/11/2026. Snapshot: `main` @ `339578e` (= `origin/main`). Diff được đọc:
`00542b6..339578e` (`e7d0ed4`, `2e0f992`, `339578e`). Người kiểm: Claude Code
(Opus 5.5). Chỉ đọc source và chạy gate trong data root tổng hợp; không sửa
source, không dùng tài khoản hoặc Zalo live.

Finding gốc: [REAUDIT_00542B6_CONSENSUS.md](REAUDIT_00542B6_CONSENSUS.md).
Gate: [VERIFICATION.md](VERIFICATION.md) mục "Tái kiểm `00542b6` và `339578e`".

## Kết luận

**CHANGES REQUIRED.** R01 (P1) đã đóng. R02 và R03 sửa đúng hướng nhưng còn tồn
tại; bản sửa R03 sinh regression P2 mới (N06). R04–R08 chưa được sửa. Gate đều
xanh, nhưng gate xanh không đóng các finding chưa có test boundary.

## Trạng thái từng finding

| ID | Trạng thái | Căn cứ |
|---|---|---|
| R01 (P1) xóa tệp gốc | **Đã đóng** | Bỏ cleanup theo tên ở `src/outgoing.rs`. Xóa giờ đòi `Batch.ephemeral` (chỉ đặt `true` cho ảnh dán clipboard, `src/gui/workspace.rs:4128`) **và** path nằm dưới `storage::account_temp_dir(owner)` hoặc `root()/temp` (`src/gui/worker.rs:5717–5733`). Picker/drop/preset đều `ephemeral: false`. |
| R02 logout TUI | **Phần lớn đã đóng** | `check_background_events` bỏ mọi event khi `logout_inflight`, chỉ xử lý `LogoutDone` đúng owner (`src/tui.rs:2416–2439`); sau logout kiểm thêm `session.is_active()`. Có test `test_tui_logout_inflight_discards_stale_events`. |
| R03 SQLite trong render | **Đóng phần render, sinh N06** | Drawer chỉ đọc `trial_peers` (`src/gui/workspace.rs:5920`). Worker nạp `list_e2ee_peers` qua `spawn_blocking` và gửi `GuiEvent::E2eePeersHydrated` có owner. |
| R04 atlas sticker | **Chưa sửa** | Không có thay đổi ở `src/gui.rs:4319–4323`, `src/gui/worker.rs:663–674`, `src/gui/messages.rs:49–50`. |
| R05 clipboard lease | **Chưa sửa** | Không có RAII/lease; sweep giữ nguyên. |
| R06 test chạm root thật | **Chưa sửa, rộng thêm** | Test mới `test_n04_list_e2ee_peers_hydrates_from_db_without_ui_thread_locks` (`tests/audit_regressions.rs`) cũng ghi E2EE DB vào root mặc định khi không có `CTDOZ_DATA_DIR`. |
| R07 memo I/O | **Chưa sửa** | `src/history_sync.rs:233,284,354,377` không đổi. |
| R08 lưu QR | **Chưa sửa** | |

## Finding mới

### N06 — P2 — Peer bị đánh dấu E2EE trong phiên không hiện trên drawer, nút gỡ bị ẩn

- Nguồn: `src/chat.rs:9030` ghi `remember_e2ee_requirement` khi tin đến mang
  metadata E2EE. Không có event nào cập nhật `trial_peers` của GUI.
- `trial_peers` chỉ được nạp tại khởi động phiên, đăng nhập QR, đổi tài khoản
  (`hydrate_e2ee_peers`, `src/gui/worker.rs`) và khi `E2eeTrialPrepared` thành
  công (`src/gui.rs:2332`).
- Hậu quả: drawer hiện trạng thái thường và ẩn hành động gỡ
  (`if !d_is_group && is_e2ee_peer`, `src/gui/workspace.rs:5932`), trong khi gửi
  plaintext cho peer đó bị từ chối. Kéo dài đến lần khởi động/đăng nhập sau.
  Trước bản sửa, drawer đọc DB nên thấy được peer này.
- Cần sửa: khi worker ghi requirement (nhánh nhận tin và nhánh gửi), phát event
  có owner để GUI thêm peer; hoặc worker làm mới snapshot khi mở drawer.
- Test cần có: tin đến đánh dấu requirement → GUI nhận event → drawer có nút gỡ.

### Tồn tại nhỏ (P3)

1. `list_e2ee_peers` nuốt lỗi DB thành danh sách rỗng
   (`src/e2ee_integration.rs`, `unwrap_or_default`). R03 yêu cầu trạng thái
   unknown/error; hiện lỗi DB làm drawer báo "thường".
2. Kết quả hydrate đến sau khi người dùng gỡ E2EE có thể thêm lại peer vừa gỡ
   (`extend` không có generation, `src/gui.rs:2341`).
3. `test_n03_outgoing_does_not_delete_user_file_named_clipboard` sao chép điều
   kiện vào test thay vì gọi `send_attachment_batch`; không bảo vệ code thật.
4. Điều kiện `root()/temp` rộng hơn cần thiết (bao cả temp toàn cục). Chỉ chạy
   khi `ephemeral`, nên không gây mất dữ liệu người dùng.
5. Ảnh clipboard gửi lỗi rồi retry thành công qua `RetryOutgoing` không được dọn;
   chỉ sweep khởi động xử lý.
6. `spawn_poll` (`src/tui.rs:1286`) vẫn không kiểm `logout_inflight`; event bị bỏ
   nhưng request mạng vẫn chạy trong lúc logout.

## Thứ tự sửa đề xuất

1. N06 (P2, regression của bản sửa).
2. R06 — cô lập data root của test trước khi thêm test mới.
3. R04, R07, R05, R08 theo [REAUDIT_00542B6_CONSENSUS.md](REAUDIT_00542B6_CONSENSUS.md).
4. Các mục P3 ở trên.

## Giới hạn

Kiểm toán source và gate x64 local + CI ARM64. Không gửi tệp thật, không đo
latency, không chứng nhận native ARM64 GPU/DPI, Hello/PIN, micro hoặc Zalo live.
