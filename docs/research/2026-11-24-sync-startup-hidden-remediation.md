# Khắc phục audit startup / sync / chỉ báo chat ẩn — 24/11/2026

Nền HEAD: `a5ad2b1`. Giữ diff UI/sidebar Antigravity có sẵn; không commit/push/release.
Đây là sửa IPC/lifecycle/local persistence/layout của fork, không đổi HTTP route,
params, cipher/signkey hoặc frame ID. Native parity chưa được chứng nhận từ test.

## Đối chiếu finding và thay đổi

| Finding | Thay đổi | Bằng chứng cần / đã thêm |
| :--- | :--- | :--- |
| Settings gửi hai lệnh, dùng sai origin | Một `SyncPhoneHistoryFenced`, owner/runtime/range rõ, worker phân loại `Setting`; cancel một lệnh fenced | Test mailbox một command, không có command thứ hai |
| Modal xác nhận không chặn nền | Shared backdrop, Foreground, input gate; Settings khóa khi chờ; Escape hủy đúng operation, giới hạn kích thước/scroll | Regression pointer parent/retry/re-enable; Escape scoped cancel |
| Retry phát operation mới khi đang chạy | Vô hiệu hóa retry khi đang chờ, có giải thích; không tự suy hợp đồng resend | Regression bấm retry không phát sync |
| Cache startup thiếu scope/freshness | Preferences/pins mang runtime/revision cho tất cả producer/consumer liên quan | Regression same-UID/old-runtime và cache revision thấp |
| Tin ẩn đã xác nhận lại nhắc khi restart | Watermark content-free trong tool_config của account DB, ghi MAX đơn điệu qua worker, echo scoped; đợi hydrate trước khi bật chỉ báo | Store in-memory monotonic/isolation; GUI bỏ echo cũ, không rewind timestamp |
| Test startup chạm network / assertion có thể rỗng | Tách load DB và publish metadata production; test dùng hai bước này cùng settle offline, bắt buộc có prerequisites event | Chạy không cần skip test network |
| Banner mô tả hardcode, CTA khác ảnh | Description từ typed remote config, suggestion CTA link style giữ hit target/wrap | Test config propagation, layout và render bounds |

Giữ timeout/gate theo origin, optional preload thật, session Expired khác Unreachable,
cache-before-Ready, privacy/hidden gates, unknown ACK và các regression trước.

## Finding bổ sung trước khi đếm sạch

Revision phải được tạo dưới AccountWritePermit sau khi đọc/ghi snapshot, không lấy
trước khi enqueue DB task. Producer có thể publish trễ; GUI và LiveUpdates mute cache
cùng từ chối revision cũ. Thứ tự lock nội bộ mute cache: revision → muted; readers
chỉ lấy muted. Khởi tạo readiness ở startup giữ session lock đến khi phát stage đầu,
tránh startup cũ xóa readiness của runtime thay thế trong khe clone/store.

Candidate cuối đã kiểm chứng:
`3fdcec24a5023039cbaf601fbcd2702ebc01893c1a80751b1941baf35d02085f`.
197 file; sort relative POSIX path, SHA256 của UTF8(path)+NUL+raw bytes+NUL cho
src/**/*.rs, mọi file tests/, Cargo.toml/Cargo.lock/build.rs/rust-toolchain.toml/
.cargo/config.toml. Tài liệu không nằm trong hash.

Trước khi chốt candidate còn phát hiện command scope được kiểm tra trước await reaping.
Đã chuyển kiểm tra sang snapshot Session lấy sau reaping cho cả recent/history fenced;
regression từ chối owner khác và relogin cùng UID/runtime mới. UI không cho cancel khi
lệnh khởi động chưa có state operation từ worker, tránh gửi cancel nhầm operation cũ.

Snapshot ghim đã được server xác nhận vẫn được phát tới UI nếu lưu cache thất bại;
cache được đánh dấu nguồn và không ghi đè snapshot authoritative trong cùng runtime.
Có regression fault-injection DB trigger để kiểm tra không làm rơi payload này.

Các build ứng viên bị thay thế đã dừng chủ động, không tính PASS hoặc lượt audit sạch.

Ứng viên 72c2752… chạy đủ 1180 test: 1168 passed, 1 failed, 11 ignored.
Test parent/retry nhận lệnh auto-preload thiết bị của trang General, không phải sync
lặp. Đã seed CallDeviceLists fixture trước khi render để cô lập pointer admission,
giữ assertion mailbox trống; sửa vị trí docstring store. Chạy lại toàn bộ trên
candidate mới ở trên, không cộng lượt sạch từ ứng viên thất bại.
## Kiểm chứng candidate cuối

- `cargo test --locked --lib -j1`: **1169 passed, 0 failed, 11 ignored**, 83.36s;
  không skip test startup; build 2m05s.
- `cargo clippy --locked --all-targets -j1 -- -D warnings`: PASS, 43.46s.
- `cargo fmt --all -- --check` và `git diff --check`: PASS.
- Chạy trực tiếp test binary đã build từ candidate này: **20 vòng × 9 regression =
  180 passed, 0 failed, 0 ignored**, luân phiên 8/16 threads; đọc và xác minh đủ
  20 summary. Không dùng exit code vòng cuối thay cho tổng kết toàn bộ.
- `cargo test --locked --test desktop_backup_v2 -j1`: **4 passed, 0 failed,
  0 ignored**, 57.34s; build 4m11s trên candidate cuối.

### Audit lượt 1 — đường đi dữ liệu và quyền sở hữu

Rà UI → command → worker admission → DB/network result → event → state → render.
Kiểm tra single-dispatch Setting/range, kiểm tra scope sau await reaping, account
permit và thứ tự tạo revision, tất cả producer/consumer preferences/pins, session
lock tại startup và watermark reset/hydrate/echo. Đối chiếu bộ test thư viện đầy
đủ và Clippy nêu trên. Không có finding mới trong phạm vi khắc phục.

### Audit lượt 2 — tình huống đối nghịch và tương tác

Rà relogin cùng UID/runtime khác, cache revision cao nhưng nguồn cached sau pins
đã authoritative, DB trigger từ chối ghi vẫn giữ payload xác nhận, cache mute cũ,
watermark đơn điệu và account-local, modal chặn parent/retry/Escape rồi re-enable.
Lặp 9 regression dưới đây 20 lần trên cùng test binary/candidate:

1. `settings_sync_dispatches_one_scoped_history_command`.
2. `phone_confirmation_blocks_settings_and_retry_without_replaying_sync`.
3. `phone_confirmation_gates_input_and_escape_only_cancels_current_operation`.
4. `startup_metadata_rejects_old_runtime_and_old_cache_revision`.
5. `hidden_unread_ack_roundtrips_monotonically_and_is_account_local`.
6. `late_cache_cannot_unmute_a_newer_live_snapshot`.
7. `phone_sync_scope_is_checked_against_the_session_after_reaping`.
8. `confirmed_server_pins_payload_survives_local_persistence_failure`.
9. `startup_preload_hydrates_preferences_and_server_pins_before_prerequisites_settled`.

Không phát hiện finding mới. Đây là hai lượt của cùng Codex với hai góc rà soát,
không phải hai reviewer độc lập. Backup integration đã qua và fingerprint cuối
trùng candidate sau tất cả kiểm tra. **Bộ đếm sạch = 2 liên tiếp**, giới hạn trong
phạm vi finding ở hồ sơ này; không chứng nhận toàn ứng dụng hoặc native parity. Log local: `%TEMP%/zalolite-sync-hidden-repair-20261124/`
(`candidate2.json`, `final2-lib.log`, `final2-clippy.log`, `final2-backup.log`,
`stress2-1.log` … `stress2-20.log`). Log tạm có thể bị dọn; test names/lệnh/hash
được giữ trong record này để chạy lại.

## Giới hạn còn giữ

- Retry/resend trong lúc chờ điện thoại chưa có contract được xác minh nên không bật.
- Readiness tổng thể cho alias, hidden/privacy/group permissions và màn hình đầu vẫn
  cần timeline/native acceptance của kế hoạch V2; không thêm sleep để giả hoàn tất.
- Màu/nhịp animation chỉ báo hidden và toàn bộ banner/native flow cần đối chiếu trực tiếp.
- Không có gửi tin/gọi hay mutation tài khoản thật trong lượt sửa này; không đóng U-T090,
  ZIP-08 hoặc task thiết bị từ unit/fixture.
