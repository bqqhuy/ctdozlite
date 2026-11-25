# Audit kết hợp session persistence và startup/backfill — 25/11/2026

## Phạm vi và quyền sở hữu

Yêu cầu người dùng: kết hợp sửa/audit chat `01a20c47-65a4-7c41-a535-41c9fbb91545`
(Fix session persistence on exit) và vòng startup/Settings hiện tại; sửa rồi audit đến
hai lượt sạch liên tiếp trên cùng candidate. Không commit/push/release.

Đã read_thread, đọc lịch sử audit và nhận bàn giao sau khi người dùng cho phép nhắn.
Chat tham chiếu đã xác nhận nhả quyền sửa; không có hai writer/build owner cùng lúc.
Candidate bàn giao: `5232951140c91b45c3683de3f08cd6c577cfd071de30323c42a341e26f64165c`.
Library đang chạy của candidate đó kết thúc: **1189 passed, 0 failed, 11 ignored**,
87.91s tại `%TEMP%/zalolite-startup-loop-lib-20261125.log`. Không dùng nó chứng nhận
candidate mới bên dưới và không cộng lượt sạch từ các vòng trước.

## Nội dung được kết hợp

- Session root: commit `802893b` đã có trong HEAD `a5ad2b1`; tên EXE/thư mục/profile
  chứa test không đổi root theo PID. Kiểm tra thêm storage_root_selection và
  storage_lifecycle trên binary hiện tại; không đọc/khôi phục credential thật từ root cũ.
- Startup: luôn refresh baseline theo runtime, cache là fallback bảo toàn; không dùng
  tỷ lệ có preview/tên để chứng nhận đủ nguồn server. Giữ ba nguồn độc lập, budget
  riêng và payload hữu ích khi nguồn khác lỗi.
- Preview: kiểm tra envelope sau giải mã, không coi refusal/ciphertext không đọc được
  là thành công. Metadata nhóm chưa xác minh báo thiếu; cache đúng version chỉ khôi
  phục provenance display, không tái dùng send_policy.
- Boundary: baseline status được reset ở login/logout/expiry. Lỗi 600/-102 ở baseline
  hoặc retry backfill đi vào reject_expired_startup với current-session/runtime guard;
  không persist snapshot lỗi, không settle Ready và không xóa phiên thay thế.
- Giữ các sửa Settings import/export pending token, conservative startup merge,
  hidden watermark, cache metadata, account write permit và origin sync đã có.

## Finding bổ sung của lượt tiếp nhận

CODE: `resolve_group_metadata` chỉ xử lý lỗi ở envelope ngoài; `if let Ok(info)` ở
mỗi gridInfoMap entry bỏ qua expiry nằm trong entry. `response_data` chỉ đọc as_i64,
nên mã lỗi dạng numeric string không đi vào phân loại expiry hiện có.

Sửa: response_data dùng integer parser chuẩn của repo và từ chối code có mặt nhưng
sai kiểu. Per-group refusal 600/-102 được trả về tới group source/baseline và đường
reject có fencing; lỗi không phải expiry vẫn giữ membership/Partial theo policy cũ.
Không đổi route, params, signing/cipher hoặc frame IDs. Regression production:
`baseline_nested_group_expiry_is_not_swallowed_or_persisted` kiểm tra numeric 600 và
string -102 qua localhost sources và thật sự gọi group metadata, không chỉ dựng enum.

## Bằng chứng và candidate

Parity với PC/Web ở lifecycle: `re-notes/01_login.md`, `re-notes/73-sync-origin-timeouts-and-startup-completion.md`
và hồ sơ preload 24/11. Đây là sửa nội bộ parsing/error/ownership, không nâng thành
E1/native hoặc chứng nhận lịch sử/cloud đầy đủ. Bản ghi storage và startup ngày trước
là nguồn chi tiết; giữ nguyên lịch sử các lần test thất bại và candidate superseded.

Candidate mới: `271610c1f91ad9859bc782830fe683cbca7b86650d2afe7b7433956fbae3db87`.
197 file; sorted relative POSIX path UTF8+NUL+raw bytes+NUL cho src/**/*.rs, tests/**
files, Cargo.toml/Cargo.lock/build.rs/rust-toolchain.toml/.cargo/config.toml.
Docs ngoài hash. Logs: `%TEMP%/zalolite-combined-session-audit-20261125/`.

Gate đã hoàn tất; hai lượt audit scoped liên tiếp không có finding mới trên cùng fingerprint. Native đóng/mở GUI với tài khoản thật,
IME/DPI, Web live/phone transfer và toàn bộ UXS còn thiếu vẫn pending. Không sửa profile
hoặc persistence option của tài khoản thật để thử.


Finding bổ sung trước khi đếm sạch: get_mini_profiles bỏ envelope sau giải mã,
resolve_thread_names bỏ lỗi enrichment. Đã dùng response_data chuẩn; thêm resolver
checked cho startup và giữ wrapper cũ cho caller best-effort khác. BaselineSourcesStatus
có enrichment riêng (không gán lỗi profile sang nguồn danh bạ); lỗi optional báo Partial,
expiry vẫn đi vào rejection trước persist/Ready. Regression
baseline_profile_refusals_remain_visible_after_decryption chạy cả success, numeric600,
string-102 và ordinary240; dữ liệu hợp lệ được giữ khi lỗi không phải expiry.


Guard persistence bổ sung: is_expired_error chỉ đọc marker server đầu tiên. Lỗi500
có mô tả nhắc mã600/-102 không còn thu hồi phiên; expiry_requires_exact_server_code
kiểm tra cả hai kiểu marker và context. Đây là phân loại lỗi local, không đổi policy
server. Các pipeline validation của candidate superseded đã được dừng có xác minh
PID/root command thuộc lượt này; không tính PASS hoặc clean audit. Gate cuối nằm ở
verified-lib.log, verified-clippy.log, verified-integration.log, verified.json.


Finding race GUI: ba producer refresh/poll/send kiểm tra Session trước khi await
xóa file nhưng event SessionExpired không có owner/runtime. UI có thể nhận event
sau khi phiên mới đã đăng nhập rồi gửi Logout cho phiên mới. Đã stamp scope từ
snapshot gốc cho cả ba producer, UI từ chối mismatch/login false. Regression
late_session_expiry_cannot_log_out_replacement_runtime kiểm tra cùng UID runtime
mới, owner khác, không phát Logout; expiry đúng phiên vẫn logout. StartupFatalError
đã có scope nên giữ cơ chế cũ. Gate của candidate cuối dùng complete-*.log/complete.json;
các gate trước bị supersede, không tính audit sạch.


Candidate aa8997c… chạy full library: 1191 passed, 1 failed, 11 ignored (59.09s).
Fixture empty-friends/valid-preview không có session key, nên checked enrichment báo
Partial thay vì Complete. Đã thêm khóa tổng hợp cho localhost fixture, không giảm
assertion Complete hoặc đổi lỗi production thành success. Candidate sau sửa fixture
ở trên được kiểm tra bằng accepted-*.log/accepted.json; kết quả cuối được chốt bên dưới.


## Kiểm chứng candidate accepted — hoàn tất

- Library `cargo test --locked --lib -j1`: **1192 passed, 0 failed, 11 ignored**,
  55.24s; build 1m31s. Không skip fixture preview, enrichment hoặc expiry.
- Stress: **20 × 13 = 260 passed, 0 failed, 0 ignored**, luân phiên 8/16 threads;
  đọc và xác minh đủ 20 summary trong accepted-stress-N.log. Selection trong
  accepted-stress-list.log; không tính test filtered out là đã chạy.
- `cargo clippy --locked --all-targets -j1 -- -D warnings`: PASS (58.06s).
- `cargo fmt --all -- --check` và `git diff --check`: PASS.
- `cargo test --locked --test storage_root_selection --test storage_lifecycle --test desktop_backup_v2 -j1`:
  **6 passed, 0 failed, 0 ignored**: backup 4, lifecycle 1, root selection 1.
  Root selection regression chạy 12 subprocess kiểm tra storage root thực tế.
- Integration có linker warning LNK4199 cho bốn Windows Media Foundation DELAYLOAD
  không được test target import. Clippy sạch; không gọi toàn bộ build là zero warnings.
- Logs cuối: `accepted-lib.log`, `accepted-clippy.log`, `accepted-integration.log`,
  `accepted-stress-list.log`, `accepted-stress-1.log` đến `accepted-stress-20.log`;
  fingerprint chuẩn trong `accepted.json`. Các verified/complete logs bên trên là
  candidate lịch sử, không phải bằng chứng nghiệm thu cuối.
- Kiểm tra lại fingerprint sau toàn bộ gates: giữ nguyên
  `271610c1f91ad9859bc782830fe683cbca7b86650d2afe7b7433956fbae3db87`.

### Lượt audit 1 — persistence, dataflow và quyền sở hữu

Đọc root selection và save/load/delete-session ownership; giữ remember-login policy,
DPAPI và root/profile selection. Truy mọi source baseline (kể cả checked enrichment),
parser/envelope refusal, persist/Partial/Ready, cached metadata provenance/send_policy,
backfill và các current-session/write-permit guards. Truy toàn bộ ba producer expiry
refresh/poll/send và GUI consumer scoped. Không có finding mới sau các sửa của
candidate accepted; full lib qua các binding/regression liên quan.

### Lượt audit 2 — phản ví dụ và kiểm tra lặp

Rà valid cache/nạp dở vẫn refresh, encrypted/string refusal, nested per-group refusal,
ordinary240 vẫn Partial giữ payload, lỗi500 có mô tả nhắc600 không logout, late expiry
cùng UID/runtime cũ hoặc owner khác không gửi Logout. Giữ expiry đúng phiên hoạt động.
Rà cache metadata đúng version không khôi phục quyền gửi; cache/UI baseline status
không sống qua account/runtime boundary. 13 regression lặp trên cùng binary/source:

- expiry_requires_exact_server_code.
- late_session_expiry_cannot_log_out_replacement_runtime.
- baseline_nested_group_expiry_is_not_swallowed_or_persisted.
- baseline_profile_refusals_remain_visible_after_decryption.
- startup_refreshes_server_baseline_and_rejects_expiry_with_warm_cache.
- baseline_pipeline_rejects_decrypted_preview_refusal.
- baseline_pipeline_preserves_membership_when_group_metadata_fails.
- baseline_group_cache_preserves_verification_without_reusing_send_permissions.
- baseline_status_is_cleared_at_account_and_runtime_boundaries.
- confirmed_startup_expiry_revokes_only_its_runtime_without_ready.
- expired_runtime_cannot_remove_newer_session_file.
- loading_older_profile_does_not_claim_newer_session_file.
- baseline_backfill_rejects_old_runtime_and_preserves_live_messages.

Không có finding mới trong phạm vi qua lượt phản ví dụ. Hai lượt ở chat tiếp nhận
cùng Codex, không phải hai reviewer cross-model độc lập. Không cộng lượt audit của
candidate bàn giao hoặc vòng sửa trước. Storage process integration và backup đều
PASS trên source accepted, fingerprint cuối không đổi. Kết luận: **2 lượt audit scoped
liên tiếp không có finding mới**. Native GUI/phone/Web vẫn pending như mục giới hạn.
Không commit, merge, push, tag hoặc release trong lượt này.
