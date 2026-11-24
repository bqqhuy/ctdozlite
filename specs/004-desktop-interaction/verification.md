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
