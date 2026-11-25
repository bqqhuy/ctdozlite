# Bằng chứng: Hiển thị, flicker và điều hướng hội thoại

Ngày ghi nhận: 2026-11-05 (Asia/Saigon). Đây là baseline từ phiên trước, không phải
kết quả chạy lại cho thay đổi tài liệu/Spec Kit. [Bản ghi phiên](../../docs/releases/v0.007-arm-work.md)
báo 565 passed, 10 ignored; Clippy/fmt/diff check đạt. Log gốc ở workspace,
không được coi là artifact phát hành; cần chạy lại ở commit chuẩn bị tag.

## Mã và fixture đã nhận diện

`src/gui/theme.rs, src/gui/workspace.rs, src/gui.rs, src/group_metadata.rs, src/history_open.rs`

- `light_custom_white_foregrounds_remain_readable`
- `truncation_preserves_combining_accents_and_joined_emoji`
- `head_cache_survives_reopens_but_is_not_permanent_coverage`
- `zero_or_malformed_server_versions_are_never_trusted`
- `tests/group_metadata.rs`

## Kết luận theo bằng chứng

Có mã và fixture cho một phần yêu cầu. Chưa xác nhận toàn tính năng trên thiết bị.
Test tương phản và grapheme không thay kiểm chứng glyph thực tế hay quan sát flicker trên GPU Windows ARM64.

## Nhật ký lần kiểm chứng tiếp theo

| Ngày / commit hoặc định danh working tree | Yêu cầu / task | Lệnh hoặc kịch bản | Kết quả và phạm vi | Giới hạn / thiết bị |
| :--- | :--- | :--- | :--- | :--- |

Chỉ thêm kết quả đã chạy; ghi test failed/ignored. Không điền bảng bằng dự đoán.
Không ghi khóa, cookie, URL có chữ ký hay nội dung tin của người khác.

## Hồi quy bố cục trước release

Native CI 41987509192 phát hiện cảnh báo quyền nhóm đẩy nút Gửi vượt viewport.
Test blocked_group_notice_keeps_send_inside_viewport tái hiện lỗi trước sửa và
qua sau khi dành chiều cao đúng galley; ba cấu hình kích thước/DPI được kiểm tra.
Chờ CI native mới; chưa đóng task nghiệm thu GUI/tài khoản thật.


## Kiểm chứng bổ sung 09/11/2026

Working tree trên `f842dfeb84fd94ff7ea79dc8d6b88dc9ad9d94bc`, theo yêu cầu sửa audit.
U-03/U-04: event lifecycle và sidebar có thay đổi liên quan được mô tả trong
[bản ghi khắc phục](../../docs/audits/2026-11-09-remediation.md).
Suite đầy đủ: 803 passed, 11 ignored; Clippy và check ARM64 đạt. Ignored được
chạy riêng; lỗi H.264 ban đầu đã có bản sửa và test native x64 chạy lại đạt.
UI smoke có PNG ở hai kích thước/theme và đã kiểm tra trực quan.
Không đóng task thiết bị hoặc chứng nhận Zalo interoperability từ các kết quả này.

## Khắc phục audit độc lập 09/11/2026 trên 164af11

Receipt E2EE schema 8 bảo vệ nhận trùng/lỗi lưu lịch sử, có kiểm tra rollback và
purge recovery copy khi xóa tin; parser phân biệt nhóm và DM. Guard API thấp giữ
cùng chính sách cho GUI/TUI. Thông báo native tôn trọng khóa và owner hiện tại;
Hello availability không chặn GUI. Registry được tạo từ login thật và lựa chọn
account được khôi phục sau restart, stale writer bị thu hồi.

Kiểm tra local: 812 test mặc định, 815 test có feature, không lỗi; 12 bài ignored
được tách riêng theo yêu cầu fixture/device/benchmark. Nguồn và giới hạn xem
[remediation](../../docs/audits/2026-11-09-independent-remediation.md), ADR-0037.
Không đóng các task nghiệm thu Zalo/tài khoản thật hoặc thiết bị ARM64 từ check x64.


## 24/11/2026 — Khắc phục audit popup/banner/attachment: hai lượt offline sạch

Nền HEAD `802893b`; giữ diff Antigravity và tài liệu Codex có trước. Phạm vi U-01/U-02
và UI-06: Settings khóa tương tác khi PIN child mở; banner tách CTA sang hàng wrap;
preview vẫn ở trong composer, tên tệp đo theo font/width, thu gọn ở ranh giới grapheme
và giữ đuôi nếu đủ chỗ. Tooltip giữ tên đầy đủ. Startup expiry được thu hồi đúng runtime,
không tiếp tục Ready; phản hồi cũ không xóa readiness của runtime mới cùng tài khoản.
Không đổi route/params/signing/cipher hoặc thêm API; đây là sửa lifecycle/layout nội bộ
của fork, không tuyên bố parity server mới.

Regression đã thêm: `pin_child_blocks_parent_settings_and_restores_input_on_escape`,
`draft_filename_fits_card_and_preserves_extension_and_graphemes`,
`configured_cta_and_all_banner_states_stay_inside_parent_width`,
`confirmed_startup_expiry_revokes_only_its_runtime_without_ready`.
Ma trận banner kiểm tra 8 kind, width 80/160/260/340, scale 1/1.5/2, ba frame.
Candidate đầu phát hiện spinner đẩy tràn ở width80; đã sửa, bộ đếm audit sạch chưa
được tính cho candidate lỗi. Candidate cố định: `fb424a38795d9e001c61adc00e0d1f01a4cf65599e1b2216a11b09f10e8630b6`.
Fingerprint gồm 197 file: src/**/*.rs, mọi file tests/, Cargo.toml/Cargo.lock/build.rs/
rust-toolchain.toml/.cargo/config.toml; sort relative POSIX path, SHA256 nối
UTF8(path)+NUL+raw bytes+NUL. Tài liệu không nằm trong fingerprint.

Lượt 1: source review data-flow/ownership và suite thư viện **1158 passed, 0 failed,
11 ignored**, 128.16s; fmt/diff-check đạt; clippy --locked --all-targets -j1 -- -D warnings
exit0 không warning (4m34s gồm chờ build lock). Không có finding mới trong phạm vi sửa.

Lượt 2: rà stale expiry sau relogin cùng UID, readiness của runtime thay thế, cleanup
credentials theo runtime, parent/child Escape và re-enable, width/grapheme/extension,
CTA dài và chuỗi liền không có khoảng trắng. **20 vòng, 5 regression/vòng**, luân phiên
8/16 test threads: **100 executions passed**, xác minh đủ 20 summary. Harness egui
đọc renderer từ source cũng qua CTA liền dài và ma trận width/scale; đây là offline
source harness, không phải native app test. Không có finding mới trong phạm vi sửa. Desktop backup integration:
`cargo test --locked --test desktop_backup_v2 -j1`: **4 passed, 0 failed, 0 ignored**,
83.92s (build 8m09s gồm chờ lock). Hash nguồn/test sau tất cả kiểm tra trùng candidate.
**Bộ đếm sạch = 2 liên tiếp**, cùng Codex theo hai góc kiểm tra, không phải hai reviewer
độc lập. Kết luận giới hạn trong bốn finding; không chứng nhận toàn ứng dụng/native.

Log cục bộ: `%TEMP%/zalolite-audit-20261124/fixed-lib.log`, `fixed-clippy.log`,
`fixed-backup.log`, `fixed-candidate.json`, `round2-1.log` đến `round2-20.log`. File tạm có thể bị dọn;
record giữ test names, lệnh, phạm vi và fingerprint để tái chạy.

Chưa nghiệm thu thao tác native/DPI/IME/tài khoản thật; U-T090 và UI-T007 vẫn mở.


## 24/11/2026 — Khắc phục audit startup/banner/hidden, nền a5ad2b1

Chi tiết phạm vi, finding và fingerprint: [hồ sơ sửa](../../docs/research/2026-11-24-sync-startup-hidden-remediation.md).
Giữ nguyên các task native còn mở. Candidate `3fdcec24a5023039cbaf601fbcd2702ebc01893c1a80751b1941baf35d02085f`
(hash 197 file source/test/toolchain theo hồ sơ): library **1169 passed, 0 failed,
11 ignored**; Clippy all-targets `-D warnings`, fmt và diff-check PASS; desktop backup
**4 passed**; stress **20 × 9 = 180 passed**, luân phiên 8/16 threads. Hai lượt audit
liên tiếp không finding mới trong phạm vi, cùng Codex theo hai góc kiểm tra; hash
không đổi sau các lệnh. Không cộng lượt sạch của ứng viên thất bại hoặc đợt trước.
Không đóng task native/readiness toàn diện từ kiểm chứng offline này.


## 25/11/2026 — Triển khai Phương án Codex P1/P2/P3 & Audit Kép Độc lập (nền a5ad2b1)

Thực hiện theo [Codex Implementation Plan](../../docs/research/2026-11-24-codex-ui-ux-settings-implementation-plan.md) (UXS-00 .. UXS-04):
- **P1**: Thích ứng Settings sidebar 48px icon-only khi compact (<700px), tiêu đề header menu_button chọn tab có chữ tiếng Việt đầy đủ cho màn hình cảm ứng/pen ARM64 (tránh hover-only), bọc `ScrollArea::vertical()` cho toàn bộ nội dung với `TextWrapMode::Wrap` và giới hạn `content_w`, giữ cơ chế khóa modal backdrop/child PIN dialog.
- **P2**: Di chuyển trực tiếp Export SQLite và Import (file/thư mục kèm xác nhận `import_owned`) vào `SettingsTab::DataManagement` có single-flight guard (`data_op_busy`), kiểm tra kết quả gửi lệnh `tools_command(...) -> bool`, và tự động nạp lại `threads` sau khi nhập dữ liệu thành công; chuyển hướng nút ở tab `Sync` sang thẳng `DataManagement`; bảo lưu entry Kho Media và zCloud.
- **P3**: Downcast `reqwest::Error` (`is_connect()` / `is_timeout()`) và `RequestDeadlineElapsed` trong `fetch_baseline_startup_threads_reported` để phân loại chính xác `BaselineFetchStatus::Offline` thay vì nhầm thành `Failed`; tối ưu hóa `WorkerCmd::RefreshThreads` bảo toàn nạp nhanh pins/preferences theo phiên hiện tại trong khi cản trùng lặp poll mạng.

Kiểm chứng tự động B3:
- `cargo fmt --all -- --check`: PASS
- `cargo clippy --locked --all-targets -- -D warnings`: PASS (0 warnings)
- `cargo test --locked --lib`: **1173 passed, 0 failed, 11 ignored** (bao gồm `settings_tabs_fit_small_wide_and_high_dpi_viewports` phủ 10 tab × 4 scale DPI `[1.0, 1.25, 1.5, 2.0]` × 3 kích thước viewport)
- `scripts/audit-prepush.ps1`: Hard gates PASSED (CHECKED=7, HEURISTIC=1, SKIPPED=1).

Audit kép độc lập B4:
- **Hai lượt audit sạch liên tiếp** (Consecutive Clean Run #2) đạt chữ ký FINAL APPROVE từ hai dòng mô hình độc lập:
  * REVIEWER_A: `gemini-4-carbon-high` (APPROVE).
  * REVIEWER_B: `claude-opus-5-5-high` (APPROVE).
- Ranh giới nghiệm thu: Đạt chuẩn ở mức CODE & FIXTURE; nghiệm thu NATIVE/DPI/cảm ứng thực tế trên thiết bị Windows ARM64 vẫn giữ ranh giới mở theo quy định của AGENTS.md. Chưa thực hiện commit/merge/push/tag/release.


## 25/11/2026 — Khắc phục Audit Startup/Sync Parity & Per-Source Readiness (Candidate `558bcd5c1e68cd3b8a2bb051c0798c5aa81f38c7`)

Thực hiện khắc phục triệt để các finding về startup preload, cache usability và sync backfill:
1. **Tách tiến độ và kết quả từng nguồn (Per-Source Baseline)**:
   - Tách rời các tác vụ lấy dữ liệu nền: Danh bạ bạn bè (`get_friends`), Danh sách nhóm (`get_groups`), Tin nhắn xem trước gần đây (`get_last_msgs`) qua `tokio::join!` với timeout độc lập (6 giây/nguồn).
   - Cấu trúc hóa kết quả từng nguồn qua `SourceFetchStatus` (`Success`, `Empty`, `Timeout`, `Offline`, `Failed`) và tổng hợp trong `BaselineSourcesStatus`.
   - Giữ và công bố toàn bộ dữ liệu hợp lệ đã tải được ngay cả khi nguồn khác timeout hoặc lỗi; không hủy bỏ dữ liệu bạn bè/nhóm khi preview gặp sự cố mạng.
   - Luôn kiểm tra preview và nhóm độc lập với danh bạ; không suy ra "tài khoản rỗng" chỉ từ danh bạ 0 bạn bè khi người dùng có nhóm hoặc tin nhắn trong preview; không nuốt lỗi tải nhóm (báo `Partial { preview_error }`, không báo `Complete` khi nhóm thất bại).
2. **Định nghĩa dữ liệu sẵn sàng & Tiêu chuẩn Cache Usability**:
   - Thay thế hoàn toàn tiêu chí thô sơ "có ít nhất một tin nhắn thì cache usable" bằng hàm đánh giá trạng thái hoàn tất `evaluate_cache_usability(&preload)` phân biệt rõ 4 trạng thái:
     * `Cold`: Bộ nhớ đệm rỗng hoàn toàn -> Kích hoạt baseline fetch.
     * `NamesOnly`: Có hội thoại nhưng toàn bộ thiếu preview và tin nhắn -> Kích hoạt baseline fetch.
     * `PartiallyHydrated`: Có dữ liệu nhưng đa số hội thoại thiếu preview hoặc chưa giải quyết tên (tên placeholder "Người dùng ...", "Nhóm ...") -> Kích hoạt baseline fetch ngay cả khi một vài hội thoại đã có tin nhắn.
     * `Usable`: Bộ nhớ đệm đầy đủ tên đã phân giải và preview/tin nhắn hợp lệ -> Sử dụng ngay, không chặn UI.
3. **Truyền trạng thái Partial/Offline & Cơ chế Backfill có Fencing**:
   - Truyền trạng thái `BaselineFetchStatus` qua `GuiEvent::StartupPrerequisitesSettled` vào UI trước khi chuyển `Ready`, cập nhật `self.status_msg` và `self.startup_baseline_status` để phản ánh trung thực trạng thái dữ liệu (đầy đủ, một phần, hay ngoại tuyến).
   - Thêm lệnh `WorkerCmd::RefreshThreadsFenced { owner, runtime_id }` và cơ chế fenced background backfill: kiểm tra chặt chẽ `owner` và `runtime_id` trước khi khởi chạy và sau mọi điểm `await`, ngăn ngừa tuyệt đối việc stale write hoặc cập nhật dữ liệu sai tài khoản khi đổi phiên/đăng xuất.
4. **Kiểm chứng tự động (CODE & FIXTURE)**:
   - `cargo fmt --all -- --check`: PASS
   - `cargo clippy --locked --all-targets -- -D warnings`: PASS (0 warnings)
   - `cargo test --locked --lib`: **1178 passed, 0 failed, 11 ignored** (bổ sung 6 tests có kiểm soát mock TCP server cho pipeline startup, timeout, group error, preview error, empty friends, và fenced gating).
   - `powershell -ExecutionPolicy Bypass -File scripts/audit-prepush.ps1`: Hard gates PASSED (CHECKED=7, HEURISTIC=1, SKIPPED=1).
5. **Ranh giới nghiệm thu và Bằng chứng**:
   - **CODE & FIXTURE**: Đạt chuẩn toàn diện, không compiler warnings, không regression.
   - **NATIVE / DEVICE ACCEPTANCE**: Giữ ranh giới mở, chờ xác nhận trực quan trên thiết bị Windows 11 ARM64 thật theo quy định của AGENTS.md.
   - **Artifact bằng chứng Zalo Web**: `C:/Users/bqqhu/.gemini/antigravity/brain/78b90cc0-d032-44cb-97cc-222294cb963f/browser_evidence/d718e8b6-a841-46db-972c-bb1386c7fa24/final_step_18.png`.
   - Tuyệt đối không commit/merge/push/tag/release trong phiên này.


## 25/11/2026 — Khắc phục startup/backfill và Settings data operation

Nguồn chuẩn: [hồ sơ finding và kiểm chứng](../../docs/research/2026-11-25-startup-settings-remediation.md).
Candidate `d1d4cd5705d970bc9adee7f2cd679ae6551c4dd10b9cb28252b49bd00e079f8b`
(197 file theo fingerprint mô tả trong hồ sơ). Tách contact-source khỏi composition
get_friends cho startup; giữ contract server/caller cũ. Giữ payload đã lọc tombstone
khi persist lỗi, scope backfill, hydrate preferences/pins/ack trước Ready và typed
pending import/export với runtime/request completion. Giữ cache/live/deleted merge.

Library: **1185 passed, 0 failed, 11 ignored**. Clippy all-targets -D warnings, fmt,
diff-check PASS. Backup integration: **4 passed**. Stress **20 × 10 = 200 passed**,
luân phiên 8/16 threads và xác minh đủ summary. Hai lượt audit cùng Codex không finding
mới trong phạm vi, cùng fingerprint sau tất cả checks; không phải dual-model review.
Các candidate thất bại/đã dừng không được cộng lượt sạch. Native/Windows interaction,
điện thoại thật, readiness tổng thể và các task UXS chưa làm vẫn mở; không đóng U-T090.
Không commit/push/release trong lượt này.


## 25/11/2026 — Combined session/startup audit, sau bàn giao

Hồ sơ: [combined audit](../../docs/research/2026-11-25-session-startup-combined-audit.md).
Candidate `271610c1f91ad9859bc782830fe683cbca7b86650d2afe7b7433956fbae3db87`
(197 source/build/test files; fingerprint cuối không đổi). Đã kiểm chứng lại storage
root/session persistence từ commit 802893b; bổ sung xử lý refusal sau giải mã,
nested group expiry, checked enrichment/Partial và owner/runtime cho SessionExpired.
Không đổi endpoint, request params, signing/cipher hoặc frame IDs.

- Library: **1192 passed, 0 failed, 11 ignored**.
- Stress: **20 × 13 = 260 passed**, xác minh đủ 20 summaries; 8/16 threads.
- Storage root/lifecycle và desktop backup integration: **6 passed, 0 failed**.
- Clippy all-targets `-D warnings`, formatter và diff-check: PASS.
- Integration có LNK4199 DELAYLOAD cho Media Foundation không được test target import;
  không tuyên bố toàn bộ build zero warnings.
- Hai lượt audit scoped liên tiếp không finding mới trên cùng source, cùng Codex;
  không phải hai reviewer cross-model độc lập. Các candidate superseded không được cộng.

CODE/FIXTURE: ACCEPTED_SCOPED. Native GUI close/reopen với tài khoản thật, IME/DPI,
phone/Web và các UXS chưa triển khai vẫn pending; không đóng U-T090 hoặc chứng nhận
full parity. Không commit/merge/push/tag/release trong lượt này.
