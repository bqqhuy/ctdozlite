# Khắc phục startup/backfill và Settings data operations — 25/11/2026

Nền HEAD `a5ad2b14c6d5b6bcd3ca59ed385f75f4fbff6b0c`, working tree Antigravity/Codex
chưa commit. Giữ thay đổi có sẵn; không commit/push/release. Áp dụng Karpathy discipline,
worker/account-write rules và spec desktop/history hiện có.

## Phạm vi và nguồn

CODE: lần đọc startup mới, baseline helper, Settings DataManagement, Tools command/result
và shared startup merge tại src/gui/worker.rs, src/gui.rs, src/gui/modals.rs,
src/gui/tools_panel.rs. Đối chiếu re-notes/73 và đặc tả preload/sync 24/11 cho lifecycle;
không đổi route/frame/params/cipher hoặc tự nâng local repair thành E1/native parity.
Các budget 6s/nguồn đã có được giữ; 2s name enrichment là budget nội bộ của fork,
không khẳng định Zalo PC dùng cùng số. Không thêm sleep kéo dài màn hình đăng nhập.

## Findings và sửa

| Finding CODE | Sửa | Regression |
| --- | --- | --- |
| Outer startup deadline bỏ toàn bộ sources đã thành công khi enrichment/DB chậm | Giữ deadline từng source, bỏ deadline bao ngoài; giới hạn riêng name enrichment | baseline_slow_name_enrichment_preserves_successful_sources |
| Ghi DB lỗi làm rơi payload và có nhánh vẫn Complete | Giữ payload đã lọc tombstone, báo Partial/persist_error; nếu không đọc được tombstone thì không công bố nội dung chưa kiểm chứng | baseline_write_failure_keeps_filtered_payload_and_reports_partial |
| Backfill chỉ phát Complete, event không scope, UI còn giữ Offline/Partial sau thành công | Event StartupBaselineUpdated có owner/runtime; phát mọi outcome hợp lệ, merge conservative và cập nhật status; không settle Ready lần nữa | baseline_backfill_rejects_old_runtime_and_preserves_live_messages |
| Snapshot remote thay preload làm mất cache-only threads | Ghép cache cũ vào baseline bằng conservative cache merge trước mandatory settle | Đối chiếu production call và existing startup merge regressions |
| Startup mới bỏ preferences/pins hydration và runtime check sau DB await | Đọc cache/prefs/pins/ack/revision dưới cùng permit; publish và readiness dưới current-session check; giữ mutex lúc reset readiness đầu | startup_cache_hydrates_preferences_pins_and_ack_under_one_permit |
| Data operation busy dựa trên chuỗi status và Tools completion không có runtime/request | ToolsData command/event scoped; typed pending token qua mọi entry helper; only matching completion clears; logout clears pending | data_operation_pending_is_not_controlled_by_status_or_stale_completions |
| Lọc contact theo prefix tên có thể xóa contact hợp lệ | Bỏ bộ lọc tên khỏi baseline; identity/dedup vẫn giữ | baseline_keeps_valid_contact_with_placeholder_shaped_name |
| Dò từ network/connect/timeout trong lỗi server gây false Offline | Phân loại theo typed error cause/deadline, không suy từ copy | baseline_error_classification_uses_causes_not_server_words |

Giữ backup format/import ownership/hidden export gate, single intent và standard backend.
Không chạy native picker/import trên tài khoản thật. Worker ToolsData chỉ nhận import/export;
UI kiểm tra login/owner/runtime/token trước khi áp payload và bỏ pending.

## Candidate và kiểm chứng

SHA256: `d1d4cd5705d970bc9adee7f2cd679ae6551c4dd10b9cb28252b49bd00e079f8b`.
197 file; sorted relative POSIX path UTF8 + NUL + raw bytes + NUL:
src/**/*.rs, mọi file tests/, Cargo.toml/Cargo.lock/build.rs/rust-toolchain.toml/.cargo/config.toml.
Docs không nằm trong hash. Candidate/test source phải giữ nguyên qua hai lượt audit sạch.

Một compile trung gian bắt fixture thiếu mut, đã sửa. Candidate 57640a7… chạy
1184 passed/1 failed/11 ignored: test slow-name chưa có synthetic session key nên
encoded_params trả sớm trước HTTP. Đã cấp khóa tổng hợp chỉ cho localhost fixture,
giữ assertion Partial/payload và tăng độ trễ mock để thật sự kiểm tra budget enrichment.
Thêm assertion lỗi data operation không thay trạng thái history search đang mở. Các compile của candidate bị thay
thế được dừng chủ động, không tính PASS hoặc lượt audit sạch. Logs cuối:
%TEMP%/zalolite-remediation-20261125/final3-{lib,clippy,backup}.log và final3.json.
### Gate trên candidate cuối

- `cargo test --locked --lib -j1`: **1185 passed, 0 failed, 11 ignored**, 65.02s,
  build 2m26s. Không skip slow-name regression hoặc chạy một test thread để che lỗi.
- `cargo clippy --locked --all-targets -j1 -- -D warnings`: PASS, 1m04s.
- `cargo fmt --all -- --check` và `git diff --check`: PASS.
- Test binary trực tiếp: **20 vòng × 10 regressions = 200 passed, 0 failed,
  0 ignored**, luân phiên 8/16 threads; đọc đủ 20 summary, không chỉ log cuối.
- `cargo test --locked --test desktop_backup_v2 -j1`: **4 passed, 0 failed,
  0 ignored**, 46.31s; build 3m55s trên candidate cuối.

### Audit lượt 1 — dataflow/ownership/composition

Truy API friend-list -> source join -> bounded enrichment -> account-write/tombstone ->
persistence result -> scoped GUI publication -> conservative merge. Truy mọi caller
get_friends để giữ composition cũ ngoài startup. Kiểm tra cache preferences/pins/ack,
revision dưới permit và readiness reset/publication dưới current session lock. Truy
Settings/Tools entry -> typed pending -> ToolsData worker admission -> completion ->
UI runtime/token match; lỗi không đóng history search. Không có finding mới sau các
sửa nêu trên trong phạm vi này; kết quả full lib/Clippy xác nhận binding và regression.

### Audit lượt 2 — phản ví dụ và tích hợp

Rà same-UID old runtime, wrong owner/token, unrelated Tools completion khi data op đang
pending, lỗi SQLite sau filter, contact tên giống placeholder, chậm profile thật có
synthetic cipher key, live message mới hơn và local deleted-message IDs, parent/child
modal và Settings viewport. Lặp trên cùng binary/source, không sửa code giữa các vòng:

1. baseline_backfill_rejects_old_runtime_and_preserves_live_messages.
2. data_operation_pending_is_not_controlled_by_status_or_stale_completions.
3. baseline_error_classification_uses_causes_not_server_words.
4. startup_cache_hydrates_preferences_pins_and_ack_under_one_permit.
5. baseline_write_failure_keeps_filtered_payload_and_reports_partial.
6. baseline_keeps_valid_contact_with_placeholder_shaped_name.
7. baseline_slow_name_enrichment_preserves_successful_sources.
8. phone_confirmation_blocks_settings_and_retry_without_replaying_sync.
9. phone_confirmation_gates_input_and_escape_only_cancels_current_operation.
10. settings_tabs_fit_small_wide_and_high_dpi_viewports.

Không có finding mới từ lượt phản ví dụ. Backup integration đã qua; fingerprint sau
tất cả kiểm tra trùng candidate. **Hai lượt audit liên tiếp sạch trong phạm vi sửa**
(bộ đếm = 2). Hai lượt cùng Codex, không phải hai reviewer độc lập cross-model.
Stress logs: final3-stress-1.log … final3-stress-20.log trong thư mục TEMP nêu trên;
log tạm có thể bị dọn, test names/commands/fingerprint giữ trong record này.

## Finding thực từ regression enrichment

Candidate 07a34c… vẫn có 1184 passed/1 failed/11 ignored. Với khóa synthetic đúng,
test thực sự chờ nhưng danh bạ không trả trong budget: get_friends gọi get_groups và
resolve_thread_names bên trong. Đã tách get_friend_contacts (cùng request/parser/cipher)
cho startup; get_friends giữ composition cũ cho caller GUI/TUI khác. Ba source startup
bây giờ độc lập cả ở call graph, không chỉ ở tokio::join!. Đây là sửa nội bộ composition,
không đổi hợp đồng server. Slow-name regression phải qua toàn pipeline, không thay
bằng test enum hoặc xóa assertion.

## Giới hạn

Nghiệm thu native/DPI/IME, điện thoại thật, cloud/2FA và readiness toàn diện vẫn riêng;
không đánh dấu task thiết bị hoặc toàn UXS-00..08 hoàn tất từ vòng sửa này. Backfill là một
lần thử có giới hạn theo flow hiện hữu; không khẳng định có retry vô hạn hay coverage đầy đủ.
Hai lượt audit của cùng Codex không phải review kép cross-model độc lập.


## Vòng sửa tiếp theo yêu cầu audit hai lần — 25/11/2026

Phạm vi: startup baseline và các finding của audit 24/11; giữ các thay đổi Settings,
sidebar và data operation đang có. Hai lượt sạch của candidate cũ không được cộng
sang candidate sửa tiếp. Không commit/push/release.

Các sửa bổ sung:

- Bỏ heuristic cache đủ theo tỷ lệ preview/tên. Mỗi runtime tải lại ba nguồn baseline;
  cache chỉ là fallback và được merge bảo toàn. Không có marker completion bền vững
  nào hiện chứng minh cache đại diện đủ dữ liệu mới từ server.
- Kiểm tra envelope preview sau giải mã; lỗi/ciphertext không đọc được không còn
  được coi là nguồn thành công. Giữ friend/group payload dùng được và báo Partial.
- Membership nhóm chưa có metadata_verified báo nguồn chưa đầy đủ. Metadata cache
  đúng version giữ cờ xác minh display, nhưng không tái sử dụng send_policy.
- Xóa startup_baseline_status ở logout/expiry và runtime mới; Empty từ server không
  tuyên bố người dùng không có lịch sử cục bộ.
- Sửa hồ sơ parity: mục kiểm tra cũ thành lịch sử, ghi HEAD thực tế; dẫn artifact
  screenshot Web local và phân biệt việc tệp tồn tại với bằng chứng E1/network.

Regression mới kiểm tra production pipeline/GUI events, không chỉ kiểm tra enum:
startup_refreshes_server_baseline_and_rejects_expiry_with_warm_cache;
baseline_pipeline_rejects_decrypted_preview_refusal;
baseline_pipeline_preserves_membership_when_group_metadata_fails;
baseline_group_cache_preserves_verification_without_reusing_send_permissions;
baseline_status_is_cleared_at_account_and_runtime_boundaries.

Một lượt thử warm-startup ban đầu thất bại vì fixture trả cấu hình thành công khiến
apply_server_info canonicalize các localhost domains. Fixture đã sửa trả config
unavailable để giữ localhost cho pipeline đang kiểm tra; không thay đổi host/security
production để làm test qua. Các build trung gian dừng sau khi có sửa mới không tính PASS.

Parity với Zalo PC/Web: nguồn E2 tại re-notes/73 và mục 3.1 của hồ sơ preload 24/11.
Đây là điều chỉnh readiness/error/cache nội bộ, không đổi endpoint, request params,
wire syncType, signing/cipher hoặc tuyên bố hoàn tất media/zCloud/phone coverage.
Budget từng nguồn/enrichment giữ từ mã hiện có, không khẳng định là thông số PC.

Candidate code/test: 197 file, SHA256
`5232951140c91b45c3683de3f08cd6c577cfd071de30323c42a341e26f64165c`.
Quy tắc fingerprint giống mục Candidate ở trên; docs ngoài hash.
Kết quả kiểm tra và audit cuối sẽ ghi bên dưới sau khi thực chạy.

Lượt rà lỗi tìm thêm auth expiry giữa baseline/backfill: mã 600/-102 (kể cả sau giải mã) đi qua reject_expired_startup, không persist/publication hoặc tự retry. Guard current-session của đường reject bảo vệ runtime thay thế. Warm-startup regression kiểm tra cả hai mã bên cạnh cache đầy đủ/nạp dở.

Full library trên candidate trung gian: 1188 passed / 1 failed / 11 ignored. Regression cache nhóm phát hiện metadata_verified bị serde(skip), nên không thể copy cờ từ dữ liệu deserialize. Đã sửa khôi phục provenance từ bảng chỉ lưu metadata đã xác minh và version khớp; send_policy vẫn None. Không cộng lượt audit sạch cho candidate lỗi này.
